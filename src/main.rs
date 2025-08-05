// TODO: Extend engine to work with any number of cards and books

use clap::Parser;
use colored::Colorize;
use easy_repl::{command, CommandStatus, Repl};
use rand::{rng, seq::SliceRandom, Rng};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::io;
use std::io::Write;
use std::rc::Rc;
use std::vec::Vec;

mod card;
use crate::card::{Book, RawCard};

mod engine;
use crate::engine::{Engine, EventRequest};

mod printer;
use crate::printer::{PrettyDisplay, Printer};

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
    cards: Vec<RawCard>,
    player_type: PlayerType
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
    card: RawCard,
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
    actual_cards: HashMap<usize, HashSet<RawCard>>,
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

impl PrettyDisplay for Book {
    fn to_pretty_string(&self) -> String {
        match *self {
            Self::LowDiamonds => "LD".blue().to_string(),
            Self::HighDiamonds => "HD".blue().to_string(),
            Self::LowClubs => "LC".green().to_string(),
            Self::HighClubs => "HC".green().to_string(),
            Self::LowHearts => "LH".red().to_string(),
            Self::HighHearts => "HH".red().to_string(),
            Self::LowSpades => "LS".bright_black().to_string(),
            Self::HighSpades => "HS".bright_black().to_string(),
            Self::Eights => "E".purple().to_string(),
        }
    }
}

impl Fish {
    fn init(num_humans: u8) -> Self {
        let num_teams = 2;
        let num_players: usize = 6;
        let num_cards: usize = 54;

        // Instantiate deck and shuffle
        let mut deck = Vec::new();
        for num in 0..num_cards {
            deck.push(RawCard { num: num as u8 })
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
        for _ in 0..num_humans {
            bot_idxs.pop();
        }

        let mut players = vec![];
        for idx in 0..num_players {
            let cards = deck.drain(0..num_cards / num_players).collect::<Vec<RawCard>>();

            let mut player_type = PlayerType::Human;
            if bot_idxs.contains(&idx) {
                let mut engine = Engine::init(num_players, num_cards, idx, &cards);
                engine.update_request();
                player_type = PlayerType::Bot { engine };
            };

            players.push(Player { idx, cards, player_type });
        }

        Fish {
            teams: Rc::new(RefCell::new(teams)),
            players: Rc::new(RefCell::new(players)),
            curr_player: Rc::new(RefCell::new(rng.random_range(0..num_players))),

            num_humans: Rc::new(RefCell::new(num_humans)),
            num_players: Rc::new(RefCell::new(num_players)),
            num_cards: Rc::new(RefCell::new(num_cards)),

            game_over: Rc::new(RefCell::new(false)),
        }
    }

    fn reset(&self) {
        let new_game: Fish = Fish::init(*self.num_humans.borrow());
        self.teams.replace(new_game.teams.take());
        self.players.replace(new_game.players.take());
        self.curr_player.replace(new_game.curr_player.take());
        self.num_players.replace(new_game.num_players.take());
        self.game_over.replace(false);
    }

    fn handle_ask(&self, askee_idx: usize, card: &RawCard) -> Result<Ask, AskError> {
        if *self.game_over.borrow() { return Err(AskError::GameOver); }

        let asker_idx = *self.curr_player.borrow();
        if self.players.borrow()[asker_idx].is_bot() {
            return Err(AskError::BotTurn);
        }
        self.ask(askee_idx, card)
    }

    fn ask(&self, askee_idx: usize, card: &RawCard) -> Result<Ask, AskError> {
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

        drop(players);
        self.check_game_end();

        Ok(Ask {
            asker: asker_idx,
            askee: askee_idx,
            card: *card,
            outcome,
        })
    }

    fn handle_next(&self) -> Result<Event, NextError> {
        if *self.game_over.borrow() { return Err(NextError::GameOver); }

        let asker_idx = *self.curr_player.borrow();
        let num_players = *self.num_players.borrow();
        let mut players = self.players.borrow_mut();

        if !players[asker_idx].is_bot() {
            return Err(NextError::HumanTurn);
        }

        for declarer_idx in (0..num_players).filter(|&p| players[p].is_bot()) {
            if let EventRequest::Declare { book, guessed_cards } = players[declarer_idx].ref_engine().request.clone() {
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
                    Err(_) => panic!("Something went wrong!"),
                }
            }
            EventRequest::Declare { .. } => unreachable!(),
            EventRequest::None => panic!("Something went wrong!")
        }
    }

    fn handle_declaration(&self, declarer_idx: usize, book: Book) -> Result<Declare, DeclareError> {
        if *self.game_over.borrow() { return Err(DeclareError::GameOver); }

        let mut players = self.players.borrow_mut();
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
                println!("Player {i} has: ");
                let guessed_cards: HashSet<RawCard> = Fish::get_cards().into_iter().collect();
                if removed_cards != guessed_cards {
                    good_declaration = false;
                }
            }

            actual_cards.insert(i, removed_cards);
        }

        let mut teams = self.teams.borrow_mut();

        if good_declaration {
            teams[declarer_idx % 2].books.push(book);
            return Ok(Declare {
                declarer: declarer_idx,
                book,
                actual_cards,
                outcome: DeclareOutcome::Success,
            });
        }

        teams[(declarer_idx + 1) % 2].books.push(book);

        drop(players);
        self.check_game_end();

        Ok(Declare {
            declarer: declarer_idx,
            book,
            actual_cards,
            outcome: DeclareOutcome::Failure,
        })
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

    // Helpers
    fn curr_player(&self) -> usize {
        *self.curr_player.borrow()
    }

    fn num_humans(&self) -> usize {
        *self.num_humans.borrow() as usize
    }

    fn num_bots(&self) -> usize {
        self.num_players() - self.num_humans()
    }

    fn num_players(&self) -> usize {
        *self.num_players.borrow()
    }

    fn num_cards(&self) -> usize {
        *self.num_cards.borrow()
    }

    fn get_cards() -> Vec<RawCard> {
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        match input
            .split_whitespace()
            .map(|s| s.parse::<RawCard>())
            .collect()
        {
            Ok(cards) => cards,
            _ => {
                println!("Invalid input");
                Fish::get_cards()
            }
        }
    }
}

#[derive(Parser)]
struct Args {
    #[clap(required = false, long, default_value = "0")]
    num_humans: u8,
}

fn main() {
    let args = Args::parse();
    let game = Fish::init(args.num_humans);
    let g = &game;
    
    let printer = Printer { use_color: Rc::new(RefCell::new(true)) };
    let p = &printer;

    // Create the repl
    let mut repl = Repl::builder()
        .with_hints(false)
        .add(
            "i",
            command! { "Info", () => || {
                    println!("There are {} bot(s) and {} human(s) in the game.",
                        g.num_bots(),
                        g.num_humans()
                    );

                    println!("It is {}'s turn", 
                        p.print_player(g.curr_player(), g), 
                    );

                    for i in 0..g.num_players() {
                        println!("{} [{}]: {}", 
                            p.print_player(i, g),
                            if g.players.borrow()[i].is_bot() { "Bot" } else { "Player" },
                            p.print_hand(i, g));
                    }

                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "a",
            command! {
                "Ask a player for a card", (askee: usize, card: RawCard) => move |askee, card| {
                    match g.handle_ask(askee, &card) {
                        Ok(ask @ Ask { askee, outcome, .. }) => {
                            // Printer
                            match outcome {
                                AskOutcome::Success => {
                                    println!("{} has the {}", p.print_player(askee, g), p.to_pretty_string(&card));
                                },
                                AskOutcome::Failure => {
                                    println!("{} does not have the {}", p.print_player(askee, g), p.to_pretty_string(&card));
                                    println!("It is the turn of {}", p.print_player(askee, g));
                                }
                            }

                            // Engines
                            let players = g.players.borrow().iter().map(|p| (p.idx, p.cards.clone())).collect();
                            g.players.borrow_mut().iter_mut().for_each(|p| {
                                if p.is_bot() {
                                    p.mut_engine().update(Event::Ask(ask.clone()));
                                    p.ref_engine().assert_sanity(&players);
                                }
                            });
                        },
                        Err(AskError::BotTurn) => {
                            println!("Error: It is a bot's turn!");
                        },
                        Err(AskError::SameTeam) => {
                            println!("Error: You cannot ask someone on your team!");
                        },
                        Err(AskError::PlayerNotFound) => {
                            println!("Error: That player does not exist!");
                        },
                        Err(AskError::InvalidBook) => {
                            println!("Error: You do not have this book in your hand!");
                        },
                        Err(AskError::AlreadyOwnCard) => {
                            println!("Error: You have the card!");
                        },
                        Err(AskError::GameOver) => {
                            println!("Error: Game is already over!")
                        }
                    }
                    Ok(CommandStatus::Done)
                }
            }
        )
        .add("c", command! {
            "Constraints", (player: usize) => move |player| {
                println!("{}", p.print_constraints(player, g));
                Ok(CommandStatus::Done)
            }
        })
        .add(
            "n",
            command! { "Next",
                (iterations: usize) => move |iterations| {
                    let mut i = 0;
                    while i < iterations {
                        match g.handle_next() {
                            Ok(ask @ Event::Ask(Ask { asker, askee, card, outcome })) => {
                                // Printer
                                let response = match outcome { AskOutcome::Success => "YES", AskOutcome::Failure => "NO" };
                                println!("{} asked {} for {} and received {response}.",
                                    p.print_player(asker, g),
                                    p.print_player(askee, g),
                                    p.to_pretty_string(&card),
                                );

                                // Engines
                                let players = g.players.borrow().iter().map(|p| (p.idx, p.cards.clone())).collect();
                                g.players.borrow_mut().iter_mut().for_each(|p| {
                                if p.is_bot() {
                                    p.mut_engine().update(ask.clone());
                                    p.ref_engine().assert_sanity(&players);
                                }
                            });
                            },
                            Ok(declare @ Event::Declare(Declare { declarer, book, outcome, .. })) => {
                                // Printer
                                let response = match outcome { DeclareOutcome::Success => "YES", DeclareOutcome::Failure => "NO" };
                                println!("{} declared {} and received {response}.",
                                    p.print_player(declarer, g),
                                    book.to_pretty_string(),
                                );
                                // Engines
                                let players = g.players.borrow().iter().map(|p| (p.idx, p.cards.clone())).collect();
                                g.players.borrow_mut().iter_mut().for_each(|p| {
                                    if p.is_bot() {
                                        p.mut_engine().update(declare.clone());
                                        p.ref_engine().assert_sanity(&players);
                                    }
                                });
                            }
                            Err(NextError::HumanTurn) => {
                                if i > 0 {
                                    break;
                                }
                                println!("Error: It's a human's turn!");
                            }
                            Err(NextError::GameOver) => {
                                if i > 0 {
                                    break;
                                }
                                println!("Error: Game is over!");
                            }
                        }
                        i += 1;
                    }
                    if i > 1 {
                        println!("{i} iterations completed");
                    }
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "d",
            command! {
                "Declare (d lh)", (book: Book) => |book| {
                    // Printer
                    let declare = g.handle_declaration(*g.curr_player.borrow(), book);
                    match declare {
                        Ok(Declare { outcome: DeclareOutcome::Success, .. }) => {
                            println!("Successfully declared {book:?}");
                        },
                        Ok(Declare { outcome: DeclareOutcome::Failure, .. }) => {
                            println!("Did not successfully declare {book:?}");
                        },
                        Err(DeclareError::GameOver) => {
                            println!("Error: Game is already over!");
                            return Ok(CommandStatus::Done);
                        }
                    }

                    // Engines
                    let players = g.players.borrow().iter().map(|p| (p.idx, p.cards.clone())).collect();
                    g.players.borrow_mut().iter_mut().for_each(|p| {
                        if p.is_bot() {
                            p.mut_engine().update(Event::Declare(declare.as_ref().ok().unwrap().clone()));
                            p.ref_engine().assert_sanity(&players);
                        }
                    });
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "r",
            command! {
                "Reset the game", () => || {
                    g.reset();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .build()
        .expect("Failed to build REPL");

    repl.run().expect("Failed to run REPL");
}
