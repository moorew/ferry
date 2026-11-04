//! Both WebSocket implementations (axum on accept, tungstenite on dial) as one pair
//! of binary-message stream and sink.

use std::pin::Pin;

use futures_util::{Sink, SinkExt, Stream, StreamExt};

pub type FrameStream = Pin<Box<dyn Stream<Item = anyhow::Result<Vec<u8>>> + Send>>;
pub type FrameSink = Pin<Box<dyn Sink<Vec<u8>, Error = anyhow::Error> + Send>>;

pub fn from_axum(socket: axum::extract::ws::WebSocket) -> (FrameSink, FrameStream) {
    use axum::extract::ws::Message;
    let (sink, stream) = socket.split();
    let sink = sink
        .sink_map_err(anyhow::Error::from)
        .with(|b: Vec<u8>| async move { Ok::<_, anyhow::Error>(Message::Binary(b.into())) });
    let stream = stream.filter_map(|m| async move {
        match m {
            Ok(Message::Binary(b)) => Some(Ok(b.to_vec())),
            Ok(Message::Text(_)) => Some(Err(anyhow::anyhow!("text frame on a binary protocol"))),
            Ok(Message::Close(_)) => Some(Err(anyhow::anyhow!("closed by the peer"))),
            Ok(Message::Ping(_) | Message::Pong(_)) => None,
            Err(e) => Some(Err(e.into())),
        }
    });
    (Box::pin(sink), Box::pin(stream))
}

pub fn from_tungstenite<S>(
    socket: tokio_tungstenite::WebSocketStream<S>,
) -> (FrameSink, FrameStream)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use tokio_tungstenite::tungstenite::Message;
    let (sink, stream) = socket.split();
    let sink = sink
        .sink_map_err(anyhow::Error::from)
        .with(|b: Vec<u8>| async move { Ok::<_, anyhow::Error>(Message::Binary(b.into())) });
    let stream = stream.filter_map(|m| async move {
        match m {
            Ok(Message::Binary(b)) => Some(Ok(b.to_vec())),
            Ok(Message::Text(_)) => Some(Err(anyhow::anyhow!("text frame on a binary protocol"))),
            Ok(Message::Close(_)) => Some(Err(anyhow::anyhow!("closed by the peer"))),
            Ok(_) => None,
            Err(e) => Some(Err(e.into())),
        }
    });
    (Box::pin(sink), Box::pin(stream))
}
