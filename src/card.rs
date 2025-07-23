use anyhow::Result;
use colored::Colorize;

#[derive(Eq, PartialEq)]
pub enum Suit {
    Diamonds,
    Clubs,
    Hearts,
    Spades,
}

#[derive(Eq, PartialEq)]
pub enum Rank {
    Num(u8),
    Jack,
    Queen,
    King,
    Ace,
}

enum DisplayCard {
    Standard { suit: Suit, rank: Rank },
    Joker { big: bool },
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Card {
    pub num: u8,
}

#[derive(Clone, Hash, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Book {
    LowDiamonds,  // 2-7
    HighDiamonds, // 9-A
    LowClubs,
    HighClubs,
    LowHearts,
    HighHearts,
    LowSpades,
    HighSpades,
    Eights, // Some variants remove the eights
}

impl Card {
    pub fn book(&self) -> Book {
        match self.num / 6 {
            0 => Book::LowDiamonds,
            1 => Book::HighDiamonds,
            2 => Book::LowClubs,
            3 => Book::HighClubs,
            4 => Book::LowHearts,
            5 => Book::HighHearts,
            6 => Book::LowSpades,
            7 => Book::HighSpades,
            8 => Book::Eights,
            _ => panic!("Invalid card number"),
        }
    }

    fn suit(&self) -> Option<Suit> {
        if self.num >= 52 {
            None
        } else {
            Some(match self.num / 12 {
                0 => Suit::Diamonds,
                1 => Suit::Clubs,
                2 => Suit::Hearts,
                3 => Suit::Spades,
                4 => match self.num % 6 {
                    0 => Suit::Diamonds,
                    1 => Suit::Clubs,
                    2 => Suit::Hearts,
                    3 => Suit::Spades,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            })
        }
    }

    fn rank(&self) -> Option<Rank> {
        if self.num >= 52 {
            None
        } else {
            Some(match self.num / 6 {
                0 | 2 | 4 | 6 => Rank::Num(self.num % 6 + 2),
                1 | 3 | 5 | 7 => match self.num % 6 {
                    0 => Rank::Num(9),
                    1 => Rank::Num(10),
                    2 => Rank::Jack,
                    3 => Rank::Queen,
                    4 => Rank::King,
                    5 => Rank::Ace,
                    _ => unreachable!(),
                },
                8 => Rank::Num(8),
                _ => unreachable!(),
            })
        }
    }

    fn display_card(&self) -> DisplayCard {
        if self.num == 52 {
            return DisplayCard::Joker { big: false };
        }
        if self.num == 53 {
            return DisplayCard::Joker { big: true };
        }

        let suit: Suit = self.suit().unwrap();
        let rank: Rank = self.rank().unwrap();
        DisplayCard::Standard { suit, rank }
    }
}

impl DisplayCard {
    fn card(&self) -> Card {
        let num = match self {
            DisplayCard::Joker { big } => {
                if *big {
                    53
                } else {
                    52
                }
            }
            DisplayCard::Standard { suit, rank } => {
                let suit_offset = match suit {
                    Suit::Diamonds => 0,
                    Suit::Clubs => 1,
                    Suit::Hearts => 2,
                    Suit::Spades => 3,
                };

                if *rank == Rank::Num(8) {
                    return Card {
                        num: 8 * 6 + suit_offset,
                    };
                }

                let rank_index = match rank {
                    Rank::Num(n @ 2..=7) => n - 2,
                    Rank::Num(n @ 9..=10) => n - 3,
                    Rank::Jack => 8,
                    Rank::Queen => 9,
                    Rank::King => 10,
                    Rank::Ace => 11,
                    _ => panic!("Invalid rank for Standard card"),
                };

                suit_offset * 12 + rank_index
            }
        };
        Card { num }
    }
}

impl Book {
    pub fn cards(&self) -> Vec<Card> {
        let offset = *self as u8;
        let mut output = vec![];
        for i in 0..6 {
            output.push(Card {
                num: offset * 6 + i,
            });
        }
        output
    }
}

// Display
impl std::fmt::Display for Suit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Suit::Diamonds => "♦",
            Suit::Clubs => "♣",
            Suit::Hearts => "♥",
            Suit::Spades => "♠",
        };
        write!(f, "{s}")
    }
}

impl std::fmt::Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rank::Num(n) => write!(f, "{n}"),
            Rank::Jack => write!(f, "J"),
            Rank::Queen => write!(f, "Q"),
            Rank::King => write!(f, "K"),
            Rank::Ace => write!(f, "A"),
        }
    }
}

impl std::fmt::Display for DisplayCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayCard::Joker { big } => {
                if *big {
                    write!(f, "BJ")
                } else {
                    write!(f, "SJ")
                }
            }
            DisplayCard::Standard { suit, rank } => write!(f, "{rank}{suit}"),
        }
    }
}

impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_card())
    }
}

// FromStr
impl std::str::FromStr for Suit {
    type Err = ParseSuitError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "D" => Ok(Suit::Diamonds),
            "C" => Ok(Suit::Clubs),
            "H" => Ok(Suit::Hearts),
            "S" => Ok(Suit::Spades),
            _ => Err(ParseSuitError),
        }
    }
}

impl std::str::FromStr for Rank {
    type Err = ParseRankError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try to parse 2-10
        if let Ok(n) = s.parse::<u8>() {
            if (2..=10).contains(&n) {
                return Ok(Rank::Num(n));
            }
        }

        // Try to parse JQKA
        match s {
            "J" => Ok(Rank::Jack),
            "Q" => Ok(Rank::Queen),
            "K" => Ok(Rank::King),
            "A" => Ok(Rank::Ace),
            _ => Err(ParseRankError),
        }
    }
}

impl std::str::FromStr for DisplayCard {
    type Err = ParseCardError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check for jokers
        match s {
            "BJ" => return Ok(DisplayCard::Joker { big: true }),
            "SJ" => return Ok(DisplayCard::Joker { big: false }),
            _ => {}
        }

        // Try to split the string into rank and suit
        if s.len() < 2 {
            return Err(ParseCardError);
        }
        let (rank_str, suit_str) = s.split_at(s.len() - 1);
        let rank = Rank::from_str(rank_str).map_err(|_| ParseCardError)?;
        let suit = Suit::from_str(suit_str).map_err(|_| ParseCardError)?;
        Ok(DisplayCard::Standard { suit, rank })
    }
}

impl std::str::FromStr for Card {
    type Err = ParseCardError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let display_card = DisplayCard::from_str(s)?;
        Ok(display_card.card())
    }
}

impl std::str::FromStr for Book {
    type Err = ParseBookError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LowDiamonds" | "ld" => Ok(Book::LowDiamonds),
            "HighDiamonds" | "hd" => Ok(Book::HighDiamonds),
            "LowClubs" | "lc" => Ok(Book::LowClubs),
            "HighClubs" | "hc" => Ok(Book::HighClubs),
            "LowHearts" | "lh" => Ok(Book::LowHearts),
            "HighHearts" | "hh" => Ok(Book::HighHearts),
            "LowSpades" | "ls" => Ok(Book::LowSpades),
            "HighSpades" | "hs" => Ok(Book::HighSpades),
            "Eights" | "e" => Ok(Book::Eights),
            _ => Err(ParseBookError),
        }
    }
}

// Format
pub trait PrettyDisplay {
    fn to_pretty_string(&self) -> String;
}

impl PrettyDisplay for Card {
    fn to_pretty_string(&self) -> String {
        match self.display_card() {
            DisplayCard::Joker { big } => {
                if big {
                    self.to_string().blue()
                } else {
                    self.to_string().red()
                }
            }
            DisplayCard::Standard { suit, .. } => match suit {
                Suit::Diamonds => self.to_string().blue(),
                Suit::Clubs => self.to_string().green(),
                Suit::Hearts => self.to_string().red(),
                Suit::Spades => self.to_string().bright_black(),
            },
        }
        .to_string()
    }
}

impl<T: PrettyDisplay> PrettyDisplay for Vec<T> {
    fn to_pretty_string(&self) -> String {
        format!(
            "[{}]",
            self.iter()
                .map(|item| item.to_pretty_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

// Debug
impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

// Error handling
#[derive(Debug, PartialEq, Eq)]
pub struct ParseSuitError;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseRankError;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseCardError;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseBookError;

impl std::error::Error for ParseSuitError {}

impl std::error::Error for ParseRankError {}

impl std::error::Error for ParseCardError {}

impl std::error::Error for ParseBookError {}

impl std::fmt::Display for ParseSuitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse suit")
    }
}

impl std::fmt::Display for ParseRankError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse rank")
    }
}

impl std::fmt::Display for ParseCardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse card")
    }
}

impl std::fmt::Display for ParseBookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse book")
    }
}
