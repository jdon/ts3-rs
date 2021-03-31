use crate::client::{Client, RawResp};
use crate::{ParseError, Decode};
use async_trait::async_trait;
use std::convert::From;
use std::str::FromStr;
use tokio::task;

impl Client {
    // Check buf for an event key. If one is found, a new task is spawned, the event
    // is dispatched to the associated handler and true is returned. If buf does not
    // contain event data, false is returned.
    pub(crate) fn dispatch_event(&self, buf: &[u8]) -> bool {
        let c = self.clone();
        let handler = c.inner.read().unwrap().handler.clone();

        // Split of the first argument (separated by ' '). It contains the event name.
        // The rest of the buffer contains the event data. 
        let (event_name, rest): (&[u8], &[u8]);
        {
            let vec: Vec<&[u8]> = buf.splitn(2, |c| *c == b' ').collect();
            event_name = vec[0];
            rest = vec[1];
        }

        // buf contains the event data which will be moved to the event task.
        let buf = rest.to_owned();

        match event_name {
            b"cliententerview" => {
                task::spawn(async move {
                    handler.cliententerview(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifyclientleftview" => {
                task::spawn(async move {
                    handler.clientleftview(c, ClientLeftView::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifyserveredited" => {
                task::spawn(async move {
                    handler.serveredited(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifychanneldescriptionchanged" => {
                task::spawn(async move {
                    handler.channeldescriptionchanged(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifychannelpasswordchanged" => {
                task::spawn(async move {
                    handler.channelpasswordchanged(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifychannelmoved" => {
                task::spawn(async move {
                    handler.channelmoved(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifychanneledited" => {
                task::spawn(async move {
                    handler.channeledited(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifychannelcreated" => {
                task::spawn(async move {
                    handler.channelcreated(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifychanneldeleted"=> {
                task::spawn(async move {
                    handler.channeldeleted(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifyclientmoved" => {
                task::spawn(async move {
                    handler.clientmoved(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifytextmessage" => {
                task::spawn(async move {
                    handler.textmessage(c, TextMessage::decode(&buf).unwrap()).await;
                });
                true
            },
            b"notifytokenused" => {
                task::spawn(async move {
                    handler.tokenused(c, RawResp::decode(&buf).unwrap()).await;
                });
                true
            },
            _ => false,
        }
    }
}

// Dispatch the event to the appropriate handler method. This function will stay alive as
// long as the handler method executes, so this should be moved to a separate task.
pub(crate) async fn dispatch_event(c: Client, resp: RawResp, event: &str) {
    let c2 = c.clone();
    let handler = c2.inner.read().unwrap().handler.clone();

    match event {
        "notifycliententerview" => handler.cliententerview(c, resp),
        // "notifyclientleftview" => handler.clientleftview(c, resp),
        "notifyserveredited" => handler.serveredited(c, resp),
        "notifychanneldescriptionchanged" => handler.channeldescriptionchanged(c, resp),
        "notifychannelpasswordchanged" => handler.channelpasswordchanged(c, resp),
        "notifychannelmoved" => handler.channelmoved(c, resp),
        "notifychanneledited" => handler.channeledited(c, resp),
        "notifychannelcreated" => handler.channelcreated(c, resp),
        "notifychanneldeleted" => handler.channeldeleted(c, resp),
        "notifyclientmoved" => handler.clientmoved(c, resp),
        "notifytextmessage" => handler.textmessage(c, resp.into()),
        "notifytokenused" => handler.tokenused(c, resp),
        _ => unreachable!(),
    }
    .await;
}

/// All events sent by the server will be dispatched to their appropriate trait method.
/// In order to receive events you must subscribe to the events you want to receive using servernotifyregister.
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn cliententerview(&self, _client: Client, _event: RawResp) {}
    async fn clientleftview(&self, _client: Client, _event: ClientLeftView) {}
    async fn serveredited(&self, _client: Client, _event: RawResp) {}
    async fn channeldescriptionchanged(&self, _client: Client, _event: RawResp) {}
    async fn channelpasswordchanged(&self, _client: Client, _event: RawResp) {}
    async fn channelmoved(&self, _client: Client, _event: RawResp) {}
    async fn channeledited(&self, _client: Client, _event: RawResp) {}
    async fn channelcreated(&self, _client: Client, _event: RawResp) {}
    async fn channeldeleted(&self, _client: Client, _event: RawResp) {}
    async fn clientmoved(&self, _client: Client, _event: RawResp) {}
    async fn textmessage(&self, _client: Client, _event: TextMessage) {}
    async fn tokenused(&self, _client: Client, _event: RawResp) {}
}

#[derive(Default)]
pub struct ClientLeftView {
    cfid: usize,
    ctid: usize,
    reasonid: ReasonID,
    invokerid: usize,
    invokername: String,
    invokeruid: String,
    reasonmsg: String,
    bantime: usize,
    clid: usize,
}

impl Decode<ClientLeftView> for ClientLeftView {
    type Err = ();

    fn decode(buf: &[u8]) -> Result<ClientLeftView, Self::Err> {
        let mut st = ClientLeftView::default();

        for s in buf.split(|c| *c == b' ') {
            let parts: Vec<&[u8]> = s.splitn(2, |c| *c == b'=').collect();

            match *parts.get(0).unwrap() {
                b"cfid" => st.cfid = usize::decode(parts.get(1).unwrap()).unwrap(),
                b"ctid" => st.ctid = usize::decode(parts.get(1).unwrap()).unwrap(),
                b"reasonid" => st.reasonid = ReasonID::decode(parts.get(1).unwrap()).unwrap(),
                b"invokerid" => st.invokerid = usize::decode(parts.get(1).unwrap()).unwrap(),
                b"invokername" => st.invokername = String::decode(parts.get(1).unwrap()).unwrap(),
                b"invokeruid" => st.invokeruid = String::decode(parts.get(1).unwrap()).unwrap(),
                b"reasonmsg" => st.reasonmsg = String::decode(parts.get(1).unwrap()).unwrap(),
                b"bantime" => st.bantime = usize::decode(parts.get(1).unwrap()).unwrap(),
                b"clid" => st.clid = usize::decode(parts.get(1).unwrap()).unwrap(),
                _ => (),
            }
        }

        Ok(st)
    }
}

/// Defines a reason why an event happened. Used in multiple event types.
pub enum ReasonID {
    /// Switched channel themselves or joined server
    SwitchChannel = 0,
    // Moved by another client or channel
    Moved,
    // Left server because of timeout (disconnect)
    Timeout,
    // Kicked from channel
    ChannelKick,
    // Kicked from server
    ServerKick,
    // Banned from server
    Ban,
    // Left server themselves
    ServerLeave,
    // Edited channel or server
    Edited,
    // Left server due shutdown
    ServerShutdown,
}

impl Decode<ReasonID> for ReasonID {
    type Err = <u8 as Decode<u8>>::Err;

    fn decode(buf: &[u8]) -> Result<ReasonID, Self::Err> {
        use ReasonID::*;
        Ok(match u8::decode(buf)? {
            0 => SwitchChannel,
            1 => Moved,
            2 => Timeout,
            3 => ChannelKick,
            4 => ServerKick,
            5 => Ban,
            6 => ServerLeave,
            7 => Edited,
            8 => ServerShutdown,
            n => panic!("Unexpected Reasonid {}", n),
        })
    }
}

impl Default for ReasonID {
    fn default() -> ReasonID {
        ReasonID::SwitchChannel
    }
}

impl FromStr for ReasonID {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<ReasonID, Self::Err> {
        match s {
            "0" => Ok(ReasonID::SwitchChannel),
            "1" => Ok(ReasonID::Moved),
            "3" => Ok(ReasonID::Timeout),
            "4" => Ok(ReasonID::ChannelKick),
            "5" => Ok(ReasonID::ServerKick),
            "6" => Ok(ReasonID::Ban),
            "8" => Ok(ReasonID::ServerLeave),
            "10" => Ok(ReasonID::Edited),
            "11" => Ok(ReasonID::ServerShutdown),
            _ => Err(ParseError::InvalidEnum),
        }
    }
}

/// Returned from a "textmessage" event
#[derive(Clone, Debug, Default)]
pub struct TextMessage {
    pub targetmode: usize,
    pub msg: String,
    pub target: usize,
    pub invokerid: usize,
    pub invokername: String,
    pub invokeruid: String,
}

impl From<RawResp> for TextMessage {
    fn from(raw: RawResp) -> TextMessage {
        TextMessage {
            targetmode: get_field(&raw, "targetmod"),
            msg: get_field(&raw, "msg"),
            target: get_field(&raw, "target"),
            invokerid: get_field(&raw, "invokerid"),
            invokername: get_field(&raw, "invokername"),
            invokeruid: get_field(&raw, "invokeruid"),
        }
    }
}

impl Decode<TextMessage> for TextMessage {
    type Err = ();

    fn decode(buf: &[u8]) -> Result<TextMessage, Self::Err> {
        let string = std::str::from_utf8(buf).unwrap();

        // Create a instance with default values
        let mut st = TextMessage::default();

        for s in string.split(" ") {
            let parts: Vec<&str> = s.splitn(2, "=").collect();

            match *parts.get(0).unwrap() {
                "targetmode" => st.targetmode = parts.get(1).unwrap().to_string().parse().unwrap(),
                "msg" => st.msg = parts.get(1).unwrap().to_string(),
                "target" => st.target = parts.get(1).unwrap().to_string().parse().unwrap(),
                "invokerid" => st.invokerid = parts.get(1).unwrap().to_string().parse().unwrap(),
                "invokername" => st.invokername = parts.get(1).unwrap().to_string(),
                "invokeruid" => st.invokeruid = parts.get(1).unwrap().to_string(),
                _ => (),
            }
        }

        Ok(st)
    }
}

// Empty default impl for EventHandler
// Used internally as a default handler
pub(crate) struct Handler;

impl EventHandler for Handler {}

fn get_field<T>(raw: &RawResp, name: &str) -> T
where
    T: FromStr + Default,
{
    match raw.items.get(0) {
        Some(val) => match val.get(name) {
            Some(val) => match val {
                Some(val) => match T::from_str(&val) {
                    Ok(val) => val,
                    Err(_) => T::default(),
                },
                None => T::default(),
            },
            None => T::default(),
        },
        None => T::default(),
    }
}

#[macro_export]
macro_rules! dispatch_event {
    ($name:expr) => {
        ("notify" + $name).as_bytes() => handler.$name(c, resp),
    };
}
