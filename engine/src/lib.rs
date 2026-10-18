//! Browser-only pairing. Credentials are never returned to page UI or persisted.
use idevice::{
    Idevice, IdeviceError,
    afc::opcode::AfcFopenMode,
    house_arrest::HouseArrestClient,
    lockdown::LockdownClient,
    remote_pairing::{RemotePairingLockdownService, RpPairingFile},
};
use send_wrapper::SendWrapper;
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/transport.js")]
extern "C" {
    fn connectPort(port: u16) -> js_sys::Promise;
    fn readPort(id: u32, size: u32) -> js_sys::Promise;
    fn writePort(id: u32, data: &[u8]) -> js_sys::Promise;
    fn closePort(id: u32);
    fn reportStage(stage: &str);
}
type Pending = Pin<Box<dyn Future<Output = Result<JsValue, JsValue>> + Send + Sync>>;
fn pending(p: js_sys::Promise) -> Pending {
    Box::pin(SendWrapper::new(JsFuture::from(p)))
}
fn io_error(_: JsValue) -> io::Error {
    io::Error::other("USB connection interrupted")
}
struct Stream {
    id: u32,
    read: Option<Pending>,
    write: Option<(Pending, usize)>,
}
impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("USBStream")
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        closePort(self.id);
    }
}
impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if self.read.is_none() {
            self.read = Some(pending(readPort(self.id, buf.remaining() as u32)));
        }
        match self.read.as_mut().unwrap().as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                self.read = None;
                let bytes = js_sys::Uint8Array::new(&result.map_err(io_error)?).to_vec();
                if bytes.len() > buf.remaining() {
                    return Poll::Ready(Err(io::Error::other("USB read overflow")));
                }
                buf.put_slice(&bytes);
                Poll::Ready(Ok(()))
            }
        }
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.write.is_none() {
            self.write = Some((pending(writePort(self.id, buf)), buf.len()));
        }
        let (future, len) = self.write.as_mut().unwrap();
        let len = *len;
        match future.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => {
                self.write = None;
                Poll::Ready(r.map(|_| len).map_err(io_error))
            }
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        closePort(self.id);
        Poll::Ready(Ok(()))
    }
}
async fn device(port: u16) -> Result<Idevice, JsValue> {
    let id = JsFuture::from(connectPort(port))
        .await?
        .as_f64()
        .ok_or_else(|| JsValue::from_str("Invalid USB stream"))? as u32;
    Ok(Idevice::new(
        Box::new(Stream {
            id,
            read: None,
            write: None,
        }),
        "Spoofed Web Setup",
    ))
}
fn error(e: IdeviceError) -> JsValue {
    JsValue::from_str(&format!("Device service error: {e}"))
}
async fn service(
    lock: &mut LockdownClient,
    name: &str,
    pair: &idevice::pairing_file::PairingFile,
) -> Result<Idevice, JsValue> {
    let (port, ssl) = lock.start_service(name).await.map_err(error)?;
    let mut d = device(port).await?;
    if ssl {
        d.start_session(pair, false).await.map_err(error)?;
    }
    Ok(d)
}
#[wasm_bindgen]
pub async fn pair_and_deliver() -> Result<(), JsValue> {
    reportStage("trust");
    let mut lock = LockdownClient::new(device(62078).await?);
    let pair = lock
        .pair(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            Some("Spoofed Setup"),
        )
        .await
        .map_err(error)?;
    lock.start_session(&pair).await.map_err(error)?;
    reportStage("checking");
    // Check app access before creating remote credentials.
    let house =
        HouseArrestClient::new(service(&mut lock, "com.apple.mobile.house_arrest", &pair).await?);
    let mut afc = house
        .vend_documents("com.yashpattekar.myapp")
        .await
        .map_err(error)?;
    // An open app setup screen supplies a per-session nonce; never replace silently.
    let mut request = afc
        .open("Documents/spoofed-setup-request.json", AfcFopenMode::RdOnly)
        .await
        .map_err(error)?;
    let bytes = request.read_n(4097).await.map_err(error)?;
    request.close().await.map_err(error)?;
    if bytes.len() > 4096 {
        return Err(JsValue::from_str("Invalid app setup request"));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| JsValue::from_str("Invalid app setup request"))?;
    let request = js_sys::JSON::parse(&text)?;
    let nonce = js_sys::Reflect::get(&request, &"nonce".into())?
        .as_string()
        .ok_or_else(|| JsValue::from_str("Open Receive from Computer in Spoofed"))?;
    if uuid::Uuid::parse_str(&nonce).is_err() {
        return Err(JsValue::from_str("Invalid app setup session"));
    }
    lock.set_value(
        "EnableWifiDebugging",
        plist::Value::Boolean(true),
        Some("com.apple.mobile.wireless_lockdown"),
    )
    .await
    .map_err(error)?;
    reportStage("pairing");
    let hostname = format!("Spoofed-{}", uuid::Uuid::new_v4());
    let mut rp = RpPairingFile::generate(&hostname);
    // Match idevice_pair: reconnect once to commit the record on the device.
    for _ in 0..2 {
        let remote = RemotePairingLockdownService::new(
            service(
                &mut lock,
                "com.apple.dt.remotepairingdeviced.lockdown",
                &pair,
            )
            .await?,
        );
        remote
            .into_client(&hostname)
            .map_err(error)?
            .connect(&mut rp, async || "000000".to_string())
            .await
            .map_err(error)?;
    }
    reportStage("sending");
    let mut envelope = plist::Dictionary::new();
    envelope.insert("nonce".into(), nonce.into());
    envelope.insert("pairing".into(), plist::Value::Data(rp.to_bytes()));
    let mut data = Vec::new();
    plist::to_writer_xml(&mut data, &envelope)
        .map_err(|_| JsValue::from_str("Could not encode delivery"))?;
    let mut file = afc
        .open("Documents/spoofed-pairing.partial", AfcFopenMode::WrOnly)
        .await
        .map_err(error)?;
    file.write_entire(&data).await.map_err(error)?;
    file.close().await.map_err(error)?;
    afc.rename(
        "Documents/spoofed-pairing.partial",
        "Documents/spoofed-pairing.plist",
    )
    .await
    .map_err(error)?;
    // Delivery is complete; the app independently validates and consumes it.
    reportStage("delivered");
    let _ = lock.stop_session().await;
    Ok(())
}
