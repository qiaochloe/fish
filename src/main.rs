use colored::Colorize;
use easy_repl::{command, CommandStatus, Repl};
use rand::{rng, seq::SliceRandom, Rng};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::Debug;
use std::io::{self, Write};
use std::rc::Rc;
use std::vec::Vec;
mod card;
use crate::card::{Book, Card, PrettyDisplay};

#[derive(Debug)]
struct Fish {
    teams: Rc<RefCell<Vec<Team>>>,
    players: Rc<RefCell<Vec<Player>>>,
    curr_player: Rc<RefCell<usize>>,
    your_index: Rc<RefCell<usize>>,
    num_players: Rc<RefCell<usize>>,
}

#[derive(Debug)]
struct Printer {
    use_color: Rc<RefCell<bool>>,
}

impl Printer {
    fn to_pretty_string(&self, obj: &(impl Debug + PrettyDisplay)) -> String {
        if *self.use_color.borrow() {
            obj.to_pretty_string()
        } else {
            format!("{obj:?}")
        }
    }
}

#[derive(Debug)]
struct Team {
    books: Vec<Book>,
}

#[derive(Debug)]
struct Player {
    idx: usize,
    cards: Vec<Card>,
}

impl PrettyDisplay for Player {
    fn to_pretty_string(&self) -> String {
        if self.idx % 2 == 0 {
            format!("{}", format!("Player {}", self.idx).blue())
        } else {
            format!("{}", format!("Player {}", self.idx).red())
        }
    }
}

struct Ask {
    asker: usize,
    askee: usize,
    card: Card,
    outcome: AskOutcome,
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
        for idx in 0..num_players {
            let cards = deck.drain(0..num_cards).collect();
            players.push(Player { idx, cards })
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

    fn handle_ask(&self, askee_idx: usize, card: &Card) -> Result<Ask, AskError> {
        let your_index = *self.your_index.borrow();
        let curr_player = *self.curr_player.borrow();

        if curr_player != your_index {
            return Err(AskError::NotYourTurn);
        }
        self.ask(askee_idx, card)
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
                *self.curr_player.borrow_mut() = askee_idx;
                AskOutcome::Failure
            }
        };

        Ok(Ask {
            asker: asker_idx,
            askee: askee_idx,
            card: card.clone(),
            outcome,
        })
    }

    fn handle_next(&self) -> Result<Ask, NextError> {
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
                Ok(ask) => return Ok(ask),
                Err(_) => continue,
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

    // Printers
    fn print_hand(&self, player: usize, p: &Printer) -> String {
        let mut players = self.players.borrow_mut();
        players[player].cards.sort();
        p.to_pretty_string(&players[player].cards)
    }

    // TODO: Can use this to print player names etc
    fn print_player(&self, player: usize, p: &Printer) -> String {
        let players = self.players.borrow();
        p.to_pretty_string(&players[player])
    }
}

fn main() {
    let game = Fish::init();
    let g = &game;

    let printer = Printer {
        use_color: Rc::new(RefCell::new(true)),
    };
    let p = &printer;

    println!("Welcome to Fish!");
    println!("You are {}", g.print_player(g.your_index(), p));
    println!("Your cards: {}", &g.print_hand(g.your_index(), p));
    println!("It is {}'s turn", g.print_player(g.curr_player(), p));

    // Create the repl
    let mut repl = Repl::builder()
        .with_hints(false)
        .add(
            "i",
            command! { "Info", () => || {
            println!("You are {}", g.print_player(g.your_index(), p));
            println!("Your cards: {}", &g.print_hand(g.your_index(), p));
            for i in 0..g.num_players() {
                println!("{}: {}", g.print_player(i, p), g.print_hand(i, p));
            }
            println!("It is {}'s turn", g.print_player(g.curr_player(), p));
                    Ok(CommandStatus::Done)
                }},
        )
        .add(
            "a",
            command! {
                "Ask a player", (askee: usize, card: Card) => move |askee, card| {
                    match g.handle_ask(askee, &card) {
                        Ok(Ask { askee, outcome: AskOutcome::Success, .. }) => {
                            println!("{} has the {}", g.print_player(askee, p), p.to_pretty_string(&card));},
                        Ok(Ask { askee, card, outcome: AskOutcome::Failure, .. }) => {
                            println!("{} does not have the {}", g.print_player(askee, p), p.to_pretty_string(&card));
                            println!("It is the turn of {}", g.print_player(askee, p));
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
                    g.check_game_end();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "n",
            command! { "Next move",
                () => || {
                    match g.handle_next() {
                        Ok(Ask { asker, askee, card, outcome: AskOutcome::Success }) =>
                            println!("{} asked {} for {} and received YES.",
                                g.print_player(asker, p),
                                g.print_player(askee, p),
                                p.to_pretty_string(&card),
                            ),
                        Ok(Ask { asker, askee, card, outcome: AskOutcome::Failure }) =>
                            println!("{} asked {} for {} and received NO.",
                                g.print_player(asker, p),
                                g.print_player(askee, p),
                                p.to_pretty_string(&card),
                            ),
                        Err(NextError::YourTurn) => println!("Error: It's your turn!"),
                    }
                    g.check_game_end();
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "d",
            command! {
                "Declare", (book: Book) => |book| {
                    match g.handle_declaration(*g.curr_player.borrow(), book) {
                        DeclarationOutcome::Success => {
                            println!("Successfully declared {book:?}");
                        },
                        DeclarationOutcome::Failure => {
                            println!("Did not successfully declare {book:?}");
                        }
                    }
                    g.check_game_end();
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
