use easy_repl::{command, CommandStatus, Repl};
use rand::{rng, seq::SliceRandom, Rng};
use std::cell::RefCell;
use std::collections::HashSet;
use std::io::{self, Write};
use std::rc::Rc;
use std::vec::Vec;
mod card;
use crate::card::{Book, Card};

#[derive(Debug)]
struct Fish {
    teams: Rc<RefCell<Vec<Team>>>,
    players: Rc<RefCell<Vec<Player>>>,
    curr_player: Rc<RefCell<usize>>,
    your_index: Rc<RefCell<usize>>,
    num_players: Rc<RefCell<usize>>,
}

#[derive(Debug)]
struct Team {
    books: Vec<Book>,
}

#[derive(Debug)]
struct Player {
    cards: Vec<Card>,
}

enum AskResult {
    Invalid,
    No,
    Yes,
}

enum DeclaractionResult {
    No,
    Yes,
}

impl Fish {
    fn start(&self) {
        println!("Welcome to Fish!");
        println!("You are Player {}", self.your_index.borrow());
        println!(
            "Your cards: {:?}",
            self.players.borrow()[*self.your_index.borrow()].cards
        );
        println!("It is Player {}'s turn", self.curr_player.borrow());
    }

    fn init() -> Self {
        let num_teams = 2;
        let num_players = 6;
        let num_cards = 54;

        // Instantiate deck and shuffle
        let mut deck = Vec::new();
        for num in 0..num_cards {
            deck.push(Card { num })
        }
        let mut rng = rng();
        deck.shuffle(&mut rng);

        // Instantiate teams
        let mut teams = vec![];
        for _ in 0..num_teams {
            teams.push(Team { books: vec![] })
        }

        // Instantiate players
        let num_cards = deck.len() / num_players;
        let mut players = vec![];
        for _ in 0..num_players {
            let cards = deck.drain(0..num_cards).collect();
            players.push(Player { cards })
        }

        Fish {
            teams: Rc::new(RefCell::new(teams)),
            players: Rc::new(RefCell::new(players)),
            curr_player: Rc::new(RefCell::new(rng.random_range(0..num_players))),
            your_index: Rc::new(RefCell::new(rng.random_range(0..num_players))),
            num_players: Rc::new(RefCell::new(num_players)),
        }
    }

    fn reset(&self) {
        let new_game: Fish = Fish::init();
        *self.teams.borrow_mut() = new_game.teams.take();
        *self.players.borrow_mut() = new_game.players.take();
        *self.curr_player.borrow_mut() = new_game.curr_player.take();
        *self.your_index.borrow_mut() = new_game.your_index.take();
        *self.num_players.borrow_mut() = new_game.num_players.take();
        new_game.start();
    }

    fn handle_info(&self) {
        println!("You are Player {}", *self.your_index.borrow());
        println!(
            "Your cards: {:?}",
            self.players.borrow()[*self.your_index.borrow()].cards
        );
        for (i, player) in self.players.borrow().iter().enumerate() {
            println!("Player {}: {:?}", i, player.cards);
        }
        println!("It is Player {}'s turn", self.curr_player.borrow());
    }

    fn handle_ask(&self, askee_idx: usize, card: Card) -> AskResult {
        let mut curr_player = self.curr_player.borrow_mut();
        let mut players = self.players.borrow_mut();

        // Error checking
        if *curr_player != *self.your_index.borrow() {
            println!("Error: It's not your turn!");
            return AskResult::Invalid;
        }
        if askee_idx >= *self.num_players.borrow() {
            println!("Error: That player does not exist!");
            return AskResult::Invalid;
        }

        // Get the asker and askee
        let (a, b) = players.split_at_mut(std::cmp::max(*curr_player, askee_idx));
        let (asker, askee) = if askee_idx < *curr_player {
            (&mut b[0], &mut a[askee_idx])
        } else {
            (&mut a[*curr_player], &mut b[0])
        };

        // Error checking
        if !asker.cards.iter().any(|c| c.book() == card.book()) {
            println!("Error: You do not have the suit!");
            return AskResult::Invalid;
        }

        if asker.cards.contains(&card) {
            println!("Error: You have the card!");
            return AskResult::Invalid;
        }

        // Check if askee has the requested card
        // If so, move it to the asker's card list
        if let Some(index) = askee.cards.iter().position(|c| *c == card) {
            let item = askee.cards.remove(index);
            asker.cards.push(item);
            println!("Player {askee_idx} has the {card}");
            return AskResult::Yes;
        }

        println!("Player {askee_idx} does not have the {card}");
        println!("It is the turn of Player {askee_idx}");
        *curr_player = askee_idx;
        AskResult::No
    }

    fn handle_next(&self) {
        let mut curr_player = self.curr_player.borrow_mut();
        if *self.your_index.borrow() == *curr_player {
            println!("It's your turn to ask!");
            return;
        }

        *curr_player = (*curr_player + 1) % *self.num_players.borrow();
        if *self.your_index.borrow() == *curr_player {
            println!("It's your turn!")
        }
    }

    fn handle_declaration(&self, declarer_idx: usize, book: Book) -> DeclaractionResult {
        let mut players = self.players.borrow_mut();
        let mut good_declaration: bool = true;

        for (i, player) in players.iter_mut().enumerate() {
            // Remove all cards of that book from the player
            let mut removed_cards = HashSet::new();
            player.cards.retain(|card| {
                if card.book() == book {
                    removed_cards.insert(card.clone());
                    false
                } else {
                    true
                }
            });

            // Check teammates
            if i % 2 == declarer_idx % 2 {
                println!("Player {i} has: ");
                let guessed_cards: HashSet<Card> = Fish::get_cards().into_iter().collect();
                if removed_cards != guessed_cards {
                    good_declaration = false;
                }
            }
        }

        let mut teams = self.teams.borrow_mut();

        if good_declaration {
            println!("Successfully declared {book:?}");
            teams[declarer_idx % 2].books.push(book);
            return DeclaractionResult::Yes;
        }

        println!("Did not successfully declare {book:?}");
        teams[(declarer_idx + 1) % 2].books.push(book);
        DeclaractionResult::No
    }

    fn get_cards() -> Vec<Card> {
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        match input
            .split_whitespace()
            .map(|s| s.parse::<Card>())
            .collect()
        {
            Ok(cards) => cards,
            _ => {
                println!("Invalid input");
                Fish::get_cards()
            }
        }
    }

    fn check_game_end(&self) -> bool {
        for p in self.players.borrow().iter() {
            if p.cards.is_empty() {
                self.reset();
                break;
            }
        }
        false
    }
}

fn main() {
    let game = Fish::init();
    game.start();

    // Create the repl
    let game_ref = &game;
    let mut repl = Repl::builder()
        .add(
            "info",
            command! { "Info", () => || {
                game_ref.handle_info();
                Ok(CommandStatus::Done)
            }},
        )
        .add(
            "ask",
            command! {
                "Ask a player", (askee: usize, card: Card) => move |askee, card| {
                    game_ref.handle_ask(askee, card);
                    game_ref.check_game_end();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "next",
            command! { "Next move",
                () => || {
                    game_ref.handle_next();
                    game_ref.check_game_end();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "declare",
            command! {
                "Declare", (book: Book) => |book| {
                    game_ref.handle_declaration(*game_ref.curr_player.borrow(), book);
                    game_ref.check_game_end();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "reset",
            command! {
                "Reset the game", () => || {
                    game_ref.reset();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .build()
        .expect("Failed to build REPL");

    repl.run().expect("Failed to run REPL");
}
