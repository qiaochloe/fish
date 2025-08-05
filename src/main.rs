// TODO: Extend engine to work with any number of cards and books

use clap::Parser;
use rand::{rng, seq::SliceRandom, Rng};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::rc::Rc;
use std::time::Instant;
use std::vec::Vec;

mod card;
use crate::card::{Book, Card};

mod engine;
use crate::engine::{Engine, EventRequest};

mod printer;
use printer::Printer;

#[derive(Debug)]
struct Fish {
    teams: Rc<RefCell<Vec<Team>>>,
    players: Rc<RefCell<Vec<Player>>>,
    curr_player: Rc<RefCell<usize>>,

    num_players: Rc<RefCell<usize>>,
    num_humans: Rc<RefCell<u8>>,
    num_cards: Rc<RefCell<usize>>,

    game_over: Rc<RefCell<bool>>,
}

#[derive(Debug)]
struct Team {
    books: Vec<Book>,
}

#[derive(Debug)]
enum PlayerType {
    Bot { engine: Engine },
    Human,
}

#[derive(Debug)]
struct Player {
    idx: usize,
    cards: Vec<Card>,
    player_type: PlayerType,
}

impl Player {
    fn is_bot(&self) -> bool {
        matches!(self.player_type, PlayerType::Bot { .. })
    }

    fn is_human(&self) -> bool {
        matches!(self.player_type, PlayerType::Human)
    }

    fn ref_engine(&self) -> &Engine {
        match &self.player_type {
            PlayerType::Bot { engine } => engine,
            PlayerType::Human => panic!("No engine for human player!"),
        }
    }

    fn mut_engine(&mut self) -> &mut Engine {
        match &mut self.player_type {
            PlayerType::Bot { engine } => engine,
            PlayerType::Human => panic!("No engine for human player!"),
        }
    }
}

#[derive(Debug, Clone)]
struct Ask {
    asker: usize,
    askee: usize,
    card: Card,
    outcome: AskOutcome,
}

#[derive(Copy, Clone, Debug)]
enum AskOutcome {
    Success,
    Failure,
}

#[derive(Debug)]
enum AskError {
    BotTurn,
    SameTeam,
    PlayerNotFound,
    InvalidBook,
    AlreadyOwnCard,
    GameOver,
}

#[derive(Debug)]
enum NextError {
    HumanTurn,
    GameOver,
}

#[derive(Debug, Clone)]
enum Event {
    Ask(Ask),
    Declare(Declare),
}

#[derive(Debug, Clone)]
struct Declare {
    declarer: usize,
    book: Book,
    actual_cards: HashMap<usize, HashSet<Card>>,
    outcome: DeclareOutcome,
}

enum DeclareError {
    GameOver,
}

#[derive(Debug, Copy, Clone)]
enum DeclareOutcome {
    Success,
    Failure,
}

impl Fish {
    fn init() -> Self {
        let num_teams = 2;
        let num_players: usize = 6;
        let num_cards: usize = 54;

        // Instantiate deck and shuffle
        let mut deck = Vec::new();
        for num in 0..num_cards {
            deck.push(Card { num: num as u8 })
        }
        let mut rng = rng();
        deck.shuffle(&mut rng);

        // Instantiate teams
        let mut teams = vec![];
        for _ in 0..num_teams {
            teams.push(Team { books: vec![] })
        }

        // Instantiate players (humans and bots)
        let mut bot_idxs: Vec<usize> = (0..num_players).collect();
        deck.shuffle(&mut rng);

        let mut players = vec![];
        for idx in 0..num_players {
            let cards = deck
                .drain(0..num_cards / num_players)
                .collect::<Vec<Card>>();

            let mut player_type = PlayerType::Human;
            if bot_idxs.contains(&idx) {
                let mut engine = Engine::init(num_players, num_cards, idx, &cards);
                engine.update_request();
                player_type = PlayerType::Bot { engine };
            };

            players.push(Player {
                idx,
                cards,
                player_type,
            });
        }

        Fish {
            teams: Rc::new(RefCell::new(teams)),
            players: Rc::new(RefCell::new(players)),
            curr_player: Rc::new(RefCell::new(rng.random_range(0..num_players))),

            num_humans: Rc::new(RefCell::new(0)),
            num_players: Rc::new(RefCell::new(num_players)),
            num_cards: Rc::new(RefCell::new(num_cards)),

            game_over: Rc::new(RefCell::new(false)),
        }
    }

    fn reset(&self) {
        let new_game: Fish = Fish::init();
        self.teams.replace(new_game.teams.take());
        self.players.replace(new_game.players.take());
        self.curr_player.replace(new_game.curr_player.take());
        self.num_players.replace(new_game.num_players.take());
        self.game_over.replace(false);
    }

    fn ask(&self, askee_idx: usize, card: &Card) -> Result<Ask, AskError> {
        // 1. The player must ask a player from the opposing team
        // 2. The player must hold a card that is part of the requested book
        // 3. The player may not ask for a card they already hold

        let asker_idx = *self.curr_player.borrow();
        if askee_idx >= *self.num_players.borrow() {
            return Err(AskError::PlayerNotFound);
        }
        if askee_idx % 2 == asker_idx % 2 {
            return Err(AskError::SameTeam);
        }

        // Get the asker and askee
        let mut players = self.players.borrow_mut();
        let (a, b) = players.split_at_mut(std::cmp::max(asker_idx, askee_idx));
        let (asker, askee) = if askee_idx < asker_idx {
            (&mut b[0], &mut a[askee_idx])
        } else {
            (&mut a[asker_idx], &mut b[0])
        };

        if !asker.cards.iter().any(|c| c.book() == card.book()) {
            return Err(AskError::InvalidBook);
        }
        if asker.cards.contains(card) {
            return Err(AskError::AlreadyOwnCard);
        }

        // Check if askee has the requested card
        // If so, move it to the asker's card list
        let outcome = {
            if let Some(index) = askee.cards.iter().position(|c| *c == *card) {
                let item = askee.cards.remove(index);
                asker.cards.push(item);
                AskOutcome::Success
            } else {
                self.curr_player.replace(askee_idx);
                AskOutcome::Failure
            }
        };

        Ok(Ask {
            asker: asker_idx,
            askee: askee_idx,
            card: *card,
            outcome,
        })
    }

    fn handle_next(&self) -> Result<Event, NextError> {
        if *self.game_over.borrow() {
            return Err(NextError::GameOver);
        }

        let asker_idx = *self.curr_player.borrow();
        let num_players = *self.num_players.borrow();
        let mut players = self.players.borrow_mut();

        for declarer_idx in (0..num_players).filter(|&p| players[p].is_bot()) {
            if let EventRequest::Declare {
                book,
                guessed_cards,
            } = players[declarer_idx].ref_engine().request.clone()
            {
                let mut good_declaration: bool = true;
                let mut actual_cards = HashMap::new();

                for (i, player) in players.iter_mut().enumerate() {
                    // Remove all cards of that book from the player
                    let mut removed_cards = HashSet::new();
                    player.cards.retain(|card| {
                        if card.book() == book {
                            removed_cards.insert(*card);
                            false
                        } else {
                            true
                        }
                    });

                    // Check teammates
                    if i % 2 == declarer_idx % 2 {
                        let guessed_cards = &guessed_cards[&i];
                        if removed_cards != *guessed_cards {
                            good_declaration = false;
                        }
                    }

                    actual_cards.insert(i, removed_cards);
                }

                let mut teams = self.teams.borrow_mut();

                if good_declaration {
                    teams[declarer_idx % 2].books.push(book);
                    return Ok(Event::Declare(Declare {
                        declarer: declarer_idx,
                        book,
                        actual_cards,
                        outcome: DeclareOutcome::Success,
                    }));
                }

                teams[(declarer_idx + 1) % 2].books.push(book);
                return Ok(Event::Declare(Declare {
                    declarer: declarer_idx,
                    book,
                    actual_cards,
                    outcome: DeclareOutcome::Failure,
                }));
            }
        }

        match &players[asker_idx].ref_engine().request.clone() {
            EventRequest::Ask { askee, card } => {
                drop(players);
                match self.ask(*askee, &card) {
                    Ok(ask) => return Ok(Event::Ask(ask)),
                    Err(AskError::GameOver) => panic!("Game is over!"),
                    Err(err) => {
                        dbg!(err);
                        panic!("Something went wrong!")
                    }
                }
            }
            EventRequest::Declare { .. } => unreachable!(),
            EventRequest::None => panic!("Something went wrong!"),
        }
    }

    fn check_game_end(&self) -> bool {
        for p in self.players.borrow().iter() {
            if p.cards.is_empty() {
                self.game_over.replace(true);
                return true;
            }
        }
        false
    }
}

#[derive(Parser)]
struct Args {
    #[clap(long, default_value = "1000")]
    num_games: usize,
}

fn main() {
    let args = Args::parse();
    let num_games = args.num_games;
    let mut win_counts = [0f64; 2];
    let start = Instant::now();

    for i in 0..num_games {
        let game = Fish::init();

        for _ in 0..1000 {
            match game.handle_next() {
                Err(NextError::GameOver) => break,
                Ok(event) => {
                    // if let Event::Ask(Ask { card, .. }) = event {
                    //     dbg!(card);
                    // }
                    // Engines
                    let players = game
                        .players
                        .borrow()
                        .iter()
                        .map(|p| (p.idx, p.cards.clone()))
                        .collect();
                    game.players.borrow_mut().iter_mut().for_each(|p| {
                        if p.is_bot() {
                            p.mut_engine().update(event.clone());
                            p.ref_engine().assert_sanity(&players);
                        }
                    });
                    game.check_game_end();
                }
                _ => continue,
            }
        }

        let teams = game.teams.borrow();
        if teams[0].books.len() > teams[1].books.len() {
            win_counts[0] += 1f64;
        } else if teams[0].books.len() < teams[1].books.len() {
            win_counts[1] += 1f64;
        } else {
            win_counts[0] += 0.5;
            win_counts[1] += 0.5;
        }

        if (i + 1) % 100 == 0 || i + 1 == num_games {
            println!("Completed {}/{} games...", i + 1, num_games);
        }
    }

    let elapsed = start.elapsed();
    println!("--- Simulation complete ---");
    println!("Games played: {}", num_games);
    println!("Elapsed time: {:.2?}", elapsed);
    println!("Average time per game: {:.4?}", elapsed / num_games as u32);
    println!("Win counts:");
    println!(
        "  Team A: {} wins ({:.2}%)",
        win_counts[0],
        100.0 * (win_counts[0] as f64) / num_games as f64
    );
    println!(
        "  Team B: {} wins ({:.2}%)",
        win_counts[0],
        100.0 * (win_counts[1] as f64) / num_games as f64
    );
}
