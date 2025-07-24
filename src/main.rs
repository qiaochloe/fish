use colored::Colorize;
use easy_repl::{command, CommandStatus, Repl};
use rand::{rng, seq::SliceRandom, Rng};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    num_cards: Rc<RefCell<usize>>,
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

#[derive(Debug)]
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
    NotYourTurn,
    SameTeam,
    PlayerNotFound,
    InvalidBook,
    AlreadyOwnCard,
}

#[derive(Debug)]
enum NextError {
    YourTurn,
}

#[derive(Debug)]
enum Event {
    Ask(Ask),
    Declare(Declare),
}

#[derive(Debug)]
struct Declare {
    book: Book,
    outcome: DeclareOutcome,
    // guesses:
}

#[derive(Debug)]
enum DeclareOutcome {
    Success,
    Failure,
}
// Once a player is logically excluded from owning a card,
// they may only gain it again through a public event
#[derive(Debug)]
struct Hand {
    slots: Vec<Option<Constraint>>,
    excluded_cards: HashSet<Card>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Constraint {
    InBook(Book),
    IsCard(Card),
}

type HandMap = HashMap<usize, Hand>;

#[derive(Debug)]
struct Engine {
    num_players: Rc<RefCell<usize>>,
    num_cards: Rc<RefCell<usize>>,
    hand_map: Rc<RefCell<HandMap>>,
}

impl Engine {
    fn init(g: &Fish) -> Self {
        let num_players = g.num_players();
        let num_cards = g.num_cards();
        let hand_map: HandMap = (0..num_players)
            .map(|i| {
                (
                    i,
                    Hand {
                        slots: vec![None; num_cards / num_players],
                        excluded_cards: HashSet::new(),
                    },
                )
            })
            .collect();

        Engine {
            num_cards: Rc::new(RefCell::new(num_cards)),
            num_players: Rc::new(RefCell::new(num_players)),
            hand_map: Rc::new(RefCell::new(hand_map)),
        }
    }

    fn reset(&self, g: &Fish) {
        let new_engine: Engine = Engine::init(g);
        *self.num_players.borrow_mut() = new_engine.num_players.take();
        *self.num_cards.borrow_mut() = new_engine.num_cards.take();
        *self.hand_map.borrow_mut() = new_engine.hand_map.take();
    }

    fn register_hand(&self, player: usize, cards: &[Card]) {
        cards.iter().for_each(|card| self.has_card(player, *card));
    }

    fn update_constraints(&self, event: Event) {
        match event {
            Event::Ask(Ask {
                asker,
                askee,
                card,
                outcome: AskOutcome::Success,
            }) => {
                // Asker has 1 card of the book
                self.has_book(asker, card.book());
                self.remove_card(askee, card);
                self.add_card(asker, card);
            }
            Event::Ask(Ask {
                asker,
                askee,
                card,
                outcome: AskOutcome::Failure,
            }) => {
                // Asker has 1 card of the book
                // Askee does not have the card
                self.has_book(asker, card.book());
                self.not_own_card(asker, card);
                self.not_own_card(askee, card);
            }
            Event::Declare(Declare { book, .. }) => {
                // No one has cards of the book anymore
                for card in book.cards() {
                    for i in 0..*self.num_players.borrow() {
                        self.remove_card(i, card);
                    }
                }
            }
        }
        dbg!(self.hand_map.borrow());
    }

    /// Player owns book. Update a None constraint if player does not already
    /// have a card of that book or hold the OwnBook constraint
    fn has_book(&self, player: usize, book: Book) {
        let mut hand_map = self.hand_map.borrow_mut();

        let hand = hand_map.get_mut(&player).unwrap();
        hand.slots.sort_by_key(|slot| match slot {
            Some(Constraint::IsCard(_)) => 0,
            Some(Constraint::InBook(_)) => 1,
            None => 2,
        });

        for slot in hand.slots.iter_mut() {
            match slot {
                Some(Constraint::IsCard(c)) if book == c.book() => return,
                Some(Constraint::InBook(b)) if book == *b => return,
                None => {
                    *slot = Some(Constraint::InBook(book));
                    return;
                }
                _ => continue,
            }
        }
        panic!("No slot available to add book constraint");
    }

    /// Player has a card. Update constraints if there are any
    fn has_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();

        for (id, hand) in hand_map.iter_mut() {
            if *id == player {
                hand.slots.sort_by_key(|slot| match slot {
                    Some(Constraint::IsCard(_)) => 0,
                    Some(Constraint::InBook(_)) => 1,
                    None => 2,
                });
                if let Some(idx) = hand.slots.iter().position(|slot| match slot {
                    Some(Constraint::IsCard(c)) if *c == card => true,
                    Some(Constraint::InBook(b)) if *b == card.book() => true,
                    None => true,
                    _ => false,
                }) {
                    hand.slots[idx] = Some(Constraint::IsCard(card))
                } else {
                    panic!("No slot available to add card constraint");
                }
            } else {
                hand.excluded_cards.insert(card);
            }
        }
    }

    /// Add a card to one of the player's slots
    /// And add it to the excluded cards of all other players
    fn add_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();
        for (id, hand) in hand_map.iter_mut() {
            if *id == player {
                hand.slots.push(Some(Constraint::IsCard(card)));
            } else {
                hand.excluded_cards.insert(card);
            }
        }
    }

    /// Player no longer owns a card. Remove the first OwnCard constraint,
    /// OwnBook constraint, or a None constraint in that order
    fn remove_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();
        let hand = hand_map.get_mut(&player).unwrap();
        hand.slots.sort_by_key(|slot| match slot {
            Some(Constraint::IsCard(_)) => 0,
            Some(Constraint::InBook(_)) => 1,
            None => 2,
        });

        if let Some(idx) = hand.slots.iter().position(|slot| match slot {
            Some(Constraint::IsCard(c)) if *c == card => true,
            Some(Constraint::InBook(b)) if *b == card.book() => true,
            None => true,
            _ => false,
        }) {
            hand.slots.remove(idx);
        } else {
            panic!("No slot available to remove");
        }
    }

    /// Players do not own the card
    fn not_own_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();
        let hand = hand_map.get_mut(&player).unwrap();
        hand.excluded_cards.insert(card);
    }
}

impl Printer {
    // Printers
    fn print_hand(&self, player: usize, g: &Fish) -> String {
        let mut players = g.players.borrow_mut();
        players[player].cards.sort();
        self.to_pretty_string(&players[player].cards)
    }

    fn print_player(&self, player: usize, g: &Fish) -> String {
        let players = g.players.borrow();
        self.to_pretty_string(&players[player])
    }
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

        // Instantiate players
        let mut players = vec![];
        for idx in 0..num_players {
            let cards = deck.drain(0..num_cards / num_players).collect();
            players.push(Player { idx, cards })
        }

        Fish {
            teams: Rc::new(RefCell::new(teams)),
            players: Rc::new(RefCell::new(players)),
            curr_player: Rc::new(RefCell::new(rng.random_range(0..num_players))),
            your_index: Rc::new(RefCell::new(rng.random_range(0..num_players))),
            num_players: Rc::new(RefCell::new(num_players)),
            num_cards: Rc::new(RefCell::new(num_cards)),
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
            card: *card,
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

    fn handle_declaration(&self, declarer_idx: usize, book: Book) -> Declare {
        let mut players = self.players.borrow_mut();
        let mut good_declaration: bool = true;

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
                let guessed_cards: HashSet<Card> = Fish::get_cards().into_iter().collect();
                if removed_cards != guessed_cards {
                    good_declaration = false;
                }
            }
        }

        let mut teams = self.teams.borrow_mut();

        if good_declaration {
            teams[declarer_idx % 2].books.push(book);
            return Declare {
                book,
                outcome: DeclareOutcome::Success,
            };
        }

        teams[(declarer_idx + 1) % 2].books.push(book);
        Declare {
            book,
            outcome: DeclareOutcome::Failure,
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

    // Helpers
    fn your_index(&self) -> usize {
        *self.your_index.borrow()
    }

    fn your_hand(&self) -> Vec<Card> {
        self.players.borrow()[self.your_index()].cards.clone()
    }

    fn curr_player(&self) -> usize {
        *self.curr_player.borrow()
    }

    fn num_players(&self) -> usize {
        *self.num_players.borrow()
    }

    fn num_cards(&self) -> usize {
        *self.num_cards.borrow()
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
}

fn main() {
    let game = Fish::init();
    let g = &game;

    let engine = Engine::init(g);
    let e = &engine;
    e.register_hand(g.your_index(), &g.your_hand());

    let printer = Printer {
        use_color: Rc::new(RefCell::new(true)),
    };
    let p = &printer;

    println!("Welcome to Fish!");
    println!("You are {}", p.print_player(g.your_index(), g));
    println!("It is {}'s turn", p.print_player(g.curr_player(), g));
    println!("Your cards: {}", &p.print_hand(g.your_index(), g));

    // Create the repl
    let mut repl = Repl::builder()
        .with_hints(false)
        .add(
            "i",
            command! { "Info", () => || {
                    println!("You are {}", p.print_player(g.your_index(), g));
                    println!("It is {}'s turn", p.print_player(g.curr_player(), g));
                    println!("Your cards: {}", &p.print_hand(g.your_index(), g));
                    for i in 0..g.num_players() {
                        println!("{}: {}", p.print_player(i, g), p.print_hand(i, g));
                    }
                    Ok(CommandStatus::Done)
                }
            },
        )
        .add(
            "a",
            command! {
                "Ask a player for a card (a 1 QD)", (askee: usize, card: Card) => move |askee, card| {
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

                            // Engine
                            e.update_constraints(Event::Ask(ask));
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
            command! { "Next",
                () => || {
                    match g.handle_next() {
                        Ok(ask @ Ask { asker, askee, card, outcome }) => {
                            // Printer
                            let response = match outcome { AskOutcome::Success => "YES", AskOutcome::Failure => "NO" };
                            println!("{} asked {} for {} and received {response}.",
                                p.print_player(asker, g),
                                p.print_player(askee, g),
                                p.to_pretty_string(&card),
                            );

                            // Engine
                            e.update_constraints(Event::Ask(ask));

                        },
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
                "Declare (d lh)", (book: Book) => |book| {
                    // Printer
                    let declare = g.handle_declaration(*g.curr_player.borrow(), book);
                    match declare.outcome {
                        DeclareOutcome::Success => {
                            println!("Successfully declared {book:?}");
                        },
                        DeclareOutcome::Failure => {
                            println!("Did not successfully declare {book:?}");
                        }
                    }

                    // Engine
                    e.update_constraints(Event::Declare(declare));
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
                    e.reset(g);
                    Ok(CommandStatus::Done)
                }
            },
        )
        .build()
        .expect("Failed to build REPL");

    repl.run().expect("Failed to run REPL");
}
