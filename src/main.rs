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

enum AskOutcome {
    Success,
    Failure,
}

enum AskError {
    NotYourTurn,
    SameTeam,
    PlayerNotFound,
    InvalidBook,
    AlreadyOwnCard,
}

struct NextOutcome {
    asker: usize,
    askee: usize,
    card: Card,
    outcome: AskOutcome,
}

enum NextError {
    YourTurn,
}

enum DeclarationOutcome {
    Success,
    Failure,
}

impl Fish {
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
    }

    fn handle_ask(&self, askee_idx: usize, card: &Card) -> Result<AskOutcome, AskError> {
        let your_index = *self.your_index.borrow();
        let curr_player = *self.curr_player.borrow();

        if curr_player != your_index {
            return Err(AskError::NotYourTurn);
        }
        self.ask(askee_idx, card)
    }

    fn ask(&self, askee_idx: usize, card: &Card) -> Result<AskOutcome, AskError> {
        let mut curr_player = self.curr_player.borrow_mut();
        if askee_idx >= *self.num_players.borrow() {
            return Err(AskError::PlayerNotFound);
        }
        if askee_idx % 2 == *curr_player % 2 {
            return Err(AskError::SameTeam);
        }

        // Get the asker and askee
        let mut players = self.players.borrow_mut();
        let (a, b) = players.split_at_mut(std::cmp::max(*curr_player, askee_idx));
        let (asker, askee) = if askee_idx < *curr_player {
            (&mut b[0], &mut a[askee_idx])
        } else {
            (&mut a[*curr_player], &mut b[0])
        };

        if !asker.cards.iter().any(|c| c.book() == card.book()) {
            return Err(AskError::InvalidBook);
        }
        if asker.cards.contains(card) {
            return Err(AskError::AlreadyOwnCard);
        }

        // Check if askee has the requested card
        // If so, move it to the asker's card list
        if let Some(index) = askee.cards.iter().position(|c| *c == *card) {
            let item = askee.cards.remove(index);
            asker.cards.push(item);
            return Ok(AskOutcome::Success);
        }

        *curr_player = askee_idx;
        Ok(AskOutcome::Failure)
    }

    fn handle_next(&self) -> Result<NextOutcome, NextError> {
        let asker = *self.curr_player.borrow();
        let your_index = *self.your_index.borrow();
        let num_players = *self.num_players.borrow();

        if your_index == asker {
            return Err(NextError::YourTurn);
        }

        // Randomly ask a user for a card
        loop {
            let rand_user = rand::rng().random_range(0..num_players);
            let rand_card = Card {
                num: rand::rng().random_range(0..54),
            };

            match self.ask(rand_user, &rand_card) {
                Ok(outcome) => {
                    return Ok(NextOutcome {
                        asker,
                        askee: rand_user,
                        card: rand_card,
                        outcome,
                    });
                }
                Err(_) => {
                    continue;
                }
            }
        }
    }

    fn handle_declaration(&self, declarer_idx: usize, book: Book) -> DeclarationOutcome {
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
            teams[declarer_idx % 2].books.push(book);
            return DeclarationOutcome::Success;
        }

        teams[(declarer_idx + 1) % 2].books.push(book);
        DeclarationOutcome::Failure
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

    // Helpers
    fn your_index(&self) -> usize {
        *self.your_index.borrow()
    }

    fn curr_player(&self) -> usize {
        *self.curr_player.borrow()
    }

    fn num_players(&self) -> usize {
        *self.num_players.borrow()
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

    fn get_sorted_hand(&self, player: usize) -> Vec<Card> {
        let mut cards = self.players.borrow()[player].cards.clone();
        cards.sort();
        cards
    }
}

fn main() {
    let game = Fish::init();
    println!("Welcome to Fish!");
    println!("You are Player {}", game.your_index());
    println!("Your cards: {:?}", game.get_sorted_hand(game.your_index()));
    println!("It is Player {}'s turn", game.curr_player());

    // Create the repl
    let game_ref = &game;
    let mut repl = Repl::builder()
        .add(
            "info",
            command! { "Info", () => || {
                let your_index = game_ref.your_index();
                println!("You are Player {your_index}");
                println!("Your cards: {:?}", game_ref.get_sorted_hand(your_index));
                for i in 0..game_ref.num_players() {
                    println!("Player {i}: {:?}", game_ref.get_sorted_hand(i));
                }
                println!("It is Player {}'s turn", game_ref.curr_player());
                        Ok(CommandStatus::Done)
                    }},
        )
        .add(
            "ask",
            command! {
                    "Ask a player", (askee: usize, card: Card) => move |askee, card| {
                        match game_ref.handle_ask(askee, &card) {
                            Ok(AskOutcome::Success) => {
                                println!("Player {askee} has the {}", &card);},
                            Ok(AskOutcome::Failure) => {
                                println!("Player {askee} does not have the {card}");
                                println!("It is the turn of Player {askee}");
                            },
                            Err(AskError::NotYourTurn) => {
                                println!("Error: It's not your turn!");
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
                        }
                        game_ref.check_game_end();
                        Ok(CommandStatus::Done)
                    }
                },
        )
        .add(
            "next",
            command! { "Next move",
                () => || {
                    match game_ref.handle_next() {
                        Ok(NextOutcome { asker, askee, card, outcome: AskOutcome::Success }) => 
                            println!("Player {asker} asked Player {askee} for {card} and received YES."),
                        Ok(NextOutcome { asker, askee, card, outcome: AskOutcome::Failure }) => 
                            println!("Player {asker} asked Player {askee} for {card} and received NO."),
                        Err(NextError::YourTurn) => println!("Error: It's your turn!"),
                    }
                    game_ref.check_game_end();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "declare",
            command! {
                "Declare", (book: Book) => |book| {
                    match game_ref.handle_declaration(*game_ref.curr_player.borrow(), book) {
                        DeclarationOutcome::Success => {
                            println!("Successfully declared {book:?}");
                        },
                        DeclarationOutcome::Failure => {
                            println!("Did not successfully declare {book:?}");
                        }
                    }
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
