//! # TS3
//! A fully asynchronous library to interact with the TeamSpeak 3 Server query interface.
//! The commands are avaliable after connecting to a TS3 Server using a [`Client`]. Commands
//! can either be sent using the associated command or using [`Client.sent`] to send raw messages.
//!
//! # Examples
//!
//! Connect to a TS3 query interface and select a server
//! ```no_run
//! use ts3::Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     // Create a new client and connect to the server query interface
//!     let client = Client::new("localhost:10011").await?;
//!
//!     // switch to virtual server with id 1
//!     client.use_sid(1).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ```no_run
//! use ts3::client::{Client, async_trait, TextMessageTarget};
//! use ts3::event::{EventHandler, ClientEnterView};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let client = Client::new("localhost:10011").await?;
//!
//!     client.use_sid(1).await?;
//!
//!     // Assign a new event handler.
//!     client.set_event_handler(Handler);
//!
//!     tokio::signal::ctrl_c().await?;
//!     Ok(())
//! }
//!
//! pub struct Handler;
//!
//! #[async_trait]
//! impl EventHandler for Handler {
//!     async fn cliententerview(&self, client: Client, event: ClientEnterView) {
//!         println!("Client {} joined!", event.client_nickname);
//!
//!         // Send a private message to the client using "sendtextmessage".
//!         client.sendtextmessage(TextMessageTarget::Client(event.clid as usize), "Hello World!")
//!             .await.unwrap();
//!     }
//! }
//!
//! ```

extern crate self as ts3;

pub mod client;
#[cfg(feature = "client")]
pub mod event;

#[cfg(feature = "client")]
pub use client::{Client, RawResp};
#[cfg(feature = "client")]
pub use event::EventHandler;
pub use ts3_derive::Decode;

use std::{
    convert::TryFrom,
    error,
    fmt::{self, Debug, Display, Formatter, Write},
    io,
    str::{from_utf8, FromStr},
};

pub enum ParseError {
    InvalidEnum,
}

type BoxError = Box<dyn error::Error + Sync + Send>;

#[derive(Debug)]
pub enum Error {
    /// Error returned from the ts3 interface. id of 0 indicates no error.
    TS3 {
        id: u16,
        msg: String,
    },
    /// Io error from the underlying tcp stream.
    Io(io::Error),
    /// Error occured while decoding the server response.
    Decode(BoxError),
    SendError,
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::TS3 { id, msg } => write!(f, "TS3 Error {}: {}", id, msg),
            Self::Io(err) => write!(f, "Io Error: {}", err),
            Self::Decode(err) => write!(f, "Error decoding value: {}", err),
            Self::SendError => write!(f, "Failed to send command, channel closed"),
        }
    }
}

impl error::Error for Error {}

#[derive(Debug)]
pub enum DecodeError {
    UnexpectedEof,
    UnexpectedChar(char),
    UnexpectedByte(u8),
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "Unexpected end while decoding"),
            Self::UnexpectedChar(c) => write!(f, "Unexpected char decoding: {}", c),
            Self::UnexpectedByte(b) => write!(f, "Unexpected byte decoding: {}", b),
        }
    }
}

impl error::Error for DecodeError {}

/// A list of other objects that are being read from or written to the TS3 server interface.
/// It implements both `FromStr` and `ToString` as long as `T` itself also implements these traits.
#[derive(Debug, PartialEq)]
pub struct List<T> {
    items: Vec<T>,
}

impl<T> List<T> {
    /// Create a new empty list
    pub fn new() -> List<T> {
        List { items: Vec::new() }
    }

    /// Create a new list filled with the items in the `Vec`.
    pub fn from_vec(vec: Vec<T>) -> List<T> {
        List { items: vec }
    }

    /// Push an item to the end of the list
    fn push(&mut self, item: T) {
        self.items.push(item);
    }

    /// Consumes the List and returns the inner `Vec` of all items in the list.
    pub fn into_vec(self) -> Vec<T> {
        self.items
    }
}

impl<T> FromStr for List<T>
where
    T: FromStr,
{
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<List<T>, Self::Err> {
        let parts: Vec<&str> = s.split("|").collect();

        let mut list = List::new();
        for item in parts {
            match T::from_str(&item) {
                Ok(item) => list.push(item),
                Err(err) => return Err(err),
            }
        }

        Ok(list)
    }
}

impl<T> ToString for List<T>
where
    T: ToString,
{
    fn to_string(&self) -> String {
        match self.items.len() {
            0 => "".to_owned(),
            1 => self.items[0].to_string(),
            _ => {
                let mut string = String::new();
                string.push_str(&self.items[0].to_string());
                for item in &self.items[1..] {
                    string.push('|');
                    string.push_str(&item.to_string());
                }

                string
            }
        }
    }
}

/// Any type implementing `Decode` can be directly decoded from the TS3 stream.
/// It provides the complete buffer of the response from the stream.
pub trait Decode<T> {
    fn decode(buf: &[u8]) -> Result<T, BoxError>;
}

pub trait Serialize {
    fn serialize(&self, writer: &mut String);
}

/// Implement `Decode` for `Vec` as long as `T` itself also implements `Decode`.
impl<T> Decode<Vec<T>> for Vec<T>
where
    T: Decode<T>,
{
    fn decode(buf: &[u8]) -> Result<Vec<T>, BoxError> {
        // Create a new vec and push all items to it.
        // Items are separated by a '|' char and no space before/after.
        let mut list = Vec::new();
        for b in buf.split(|c| *c == b'|') {
            list.push(T::decode(&b)?);
        }
        Ok(list)
    }
}

/// Implements `Serialize` for types that can be directly written as they are formatted.
macro_rules! impl_serialize {
    ($t:ty) => {
        impl crate::Serialize for $t {
            fn serialize(&self, writer: &mut ::std::string::String) {
                write!(writer, "{}", self).unwrap();
            }
        }
    };
}

/// The `impl_decode` macro implements `Decode` for any type that implements `FromStr`.
macro_rules! impl_decode {
    ($t:ty) => {
        impl Decode<$t> for $t {
            fn decode(buf: &[u8]) -> std::result::Result<$t, BoxError> {
                Ok(from_utf8(buf)?.parse()?)
            }
        }
    };
}

#[derive(Clone, Debug, Default)]
pub struct CommandBuilder(String);

impl CommandBuilder {
    pub fn new<T>(name: T) -> Self
    where
        T: ToString,
    {
        Self(name.to_string())
    }

    pub fn arg<T, S>(mut self, key: T, value: S) -> Self
    where
        T: AsRef<str>,
        S: Serialize,
    {
        self.0.write_char(' ').unwrap();
        self.0.write_str(key.as_ref()).unwrap();
        self.0.write_char('=').unwrap();
        value.serialize(&mut self.0);
        self
    }

    pub fn arg_opt<T, S>(self, key: T, value: Option<S>) -> Self
    where
        T: AsRef<str>,
        S: Serialize,
    {
        match value {
            Some(value) => self.arg(key, value),
            None => self,
        }
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Implement `Decode` for `()`. Calling `()::decode(&[u8])` will never fail
/// and can always be unwrapped safely.
impl Decode<()> for () {
    fn decode(_: &[u8]) -> Result<(), BoxError> {
        // command!("");
        Ok(())
    }
}

// Implement `Decode` for `String`
impl Decode<String> for String {
    fn decode(buf: &[u8]) -> Result<String, BoxError> {
        // Create a new string, allocating the same length as the buffer. Most
        // chars are one-byte only.
        let mut string = String::with_capacity(buf.len());

        // Create a peekable iterator to iterate over all bytes, appending all bytes
        // and replacing escaped chars.
        let mut iter = buf.into_iter().peekable();
        while let Some(b) = iter.next() {
            match b {
                // Match any escapes, starting with a '\' followed by another char.
                b'\\' => {
                    match iter.peek() {
                        Some(c) => match c {
                            b'\\' => string.push('\\'),
                            b'/' => string.push('/'),
                            b's' => string.push(' '),
                            b'p' => string.push('|'),
                            b'a' => string.push(7u8 as char),
                            b'b' => string.push(8u8 as char),
                            b'f' => string.push(12u8 as char),
                            b'n' => string.push(10u8 as char),
                            b'r' => string.push(13u8 as char),
                            b't' => string.push(9u8 as char),
                            b'v' => string.push(11u8 as char),
                            _ => return Err(Box::new(DecodeError::UnexpectedByte(**c))),
                        },
                        None => return Err(Box::new(DecodeError::UnexpectedEof)),
                    }
                    iter.next();
                }
                _ => string.push(char::try_from(*b).unwrap()),
            }
        }

        // Shrink the string to its fitting size before returning it.
        string.shrink_to_fit();
        Ok(string)
    }
}

impl Serialize for &str {
    fn serialize(&self, writer: &mut String) {
        for c in self.chars() {
            match c {
                '\\' => writer.write_str("\\\\").unwrap(),
                '/' => writer.write_str("\\/").unwrap(),
                ' ' => writer.write_str("\\s").unwrap(),
                '|' => writer.write_str("\\p").unwrap(),
                c if c == 7u8 as char => writer.write_str("\\a").unwrap(),
                c if c == 8u8 as char => writer.write_str("\\b").unwrap(),
                c if c == 12u8 as char => writer.write_str("\\f").unwrap(),
                c if c == 10u8 as char => writer.write_str("\\n").unwrap(),
                c if c == 13u8 as char => writer.write_str("\\r").unwrap(),
                c if c == 9u8 as char => writer.write_str("\\t").unwrap(),
                c if c == 11u8 as char => writer.write_str("\\v").unwrap(),
                _ => writer.write_char(c).unwrap(),
            }
        }
    }
}

impl Serialize for bool {
    fn serialize(&self, writer: &mut String) {
        write!(
            writer,
            "{}",
            match self {
                false => b'0',
                true => b'1',
            }
        )
        .unwrap();
    }
}

impl Decode<bool> for bool {
    fn decode(buf: &[u8]) -> Result<bool, BoxError> {
        match buf.get(0) {
            Some(b) => match b {
                b'0' => Ok(true),
                b'1' => Ok(false),
                _ => Err(Box::new(DecodeError::UnexpectedByte(*b))),
            },
            None => Err(Box::new(DecodeError::UnexpectedEof)),
        }
    }
}

// Implement `Decode` for all int types.
impl_decode!(isize);
impl_decode!(i8);
impl_decode!(i16);
impl_decode!(i32);
impl_decode!(i64);
impl_decode!(i128);

impl_decode!(usize);
impl_decode!(u8);
impl_decode!(u16);
impl_decode!(u32);
impl_decode!(u64);
impl_decode!(u128);

impl_serialize!(isize);
impl_serialize!(i8);
impl_serialize!(i16);
impl_serialize!(i32);
impl_serialize!(i64);
impl_serialize!(i128);

impl_serialize!(usize);
impl_serialize!(u8);
impl_serialize!(u16);
impl_serialize!(u32);
impl_serialize!(u64);
impl_serialize!(u128);

impl Decode<Error> for Error {
    fn decode(buf: &[u8]) -> Result<Error, BoxError> {
        let (mut id, mut msg) = (0, String::new());

        // Error is a key-value map separated by ' ' with only the id and msg key.
        for s in buf.split(|c| *c == b' ') {
            // Error starts with "error" as the first key.
            if s == b"error" {
                continue;
            }

            // Get both key and value from the buffer, separated by a '='.
            let parts: Vec<&[u8]> = s.splitn(2, |c| *c == b'=').collect();

            match parts.get(0) {
                Some(key) => {
                    // Extract the value.
                    let val = match parts.get(1) {
                        Some(val) => val,
                        None => return Err(Box::new(DecodeError::UnexpectedEof)),
                    };

                    // Match the key of the pair and assign the corresponding value.
                    match *key {
                        b"id" => {
                            id = u16::decode(val)?;
                        }
                        b"msg" => {
                            msg = String::decode(val)?;
                        }
                        _ => (),
                    }
                }
                None => return Err(Box::new(DecodeError::UnexpectedEof)),
            }
        }

        Ok(Error::TS3 { id, msg })
    }
}

mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_vec_decode() {
        let buf = b"test|test2";
        assert_eq!(
            Vec::<String>::decode(buf).unwrap(),
            vec!["test".to_owned(), "test2".to_owned()]
        );
    }

    #[test]
    fn test_string_decode() {
        let buf = b"Hello\\sWorld!";
        assert_eq!(String::decode(buf).unwrap(), "Hello World!".to_owned());
    }

    #[test]
    fn test_error_decode() {
        let buf = b"error id=0 msg=ok";
        let (id, msg) = match Error::decode(buf).unwrap() {
            Error::TS3 { id, msg } => (id, msg),
            _ => unreachable!(),
        };
        assert!(id == 0 && msg == "ok".to_owned());
    }

    #[test]
    fn test_list_to_string() {
        let mut list = List::new();
        assert_eq!(list.to_string(), "");
        list.push(1);
        assert_eq!(list.to_string(), "1");
        list.push(2);
        assert_eq!(list.to_string(), "1|2");
    }

    #[test]
    fn test_list_from_str() {
        let string = "1|2|3|4";
        assert_eq!(
            List::from_str(&string).unwrap(),
            List {
                items: vec![1, 2, 3, 4]
            }
        );
    }

    #[test]
    fn test_command_builder() {
        let cmd = CommandBuilder::new("testcmd");
        assert_eq!(cmd.clone().into_inner(), "testcmd");

        let cmd = cmd.arg("hello", "world");
        assert_eq!(cmd.clone().into_inner(), "testcmd hello=world");

        let cmd = cmd.arg("test", "1234");
        assert_eq!(cmd.clone().into_inner(), "testcmd hello=world test=1234");

        let cmd = cmd.arg_opt("opt", Some("arg"));
        assert_eq!(
            cmd.clone().into_inner(),
            "testcmd hello=world test=1234 opt=arg"
        );

        let cmd = cmd.arg_opt::<_, &str>("opt2", None);
        assert_eq!(cmd.into_inner(), "testcmd hello=world test=1234 opt=arg");
    }
}
