use crate::protocol;
use idevice::{
    Idevice, IdeviceError,
    afc::{AfcClient, opcode::AfcFopenMode},
    house_arrest::HouseArrestClient,
    lockdown::LockdownClient,
    pairing_file::PairingFile,
    provider::{IdeviceProvider, UsbmuxdProvider},
    remote_pairing::{RemotePairingLockdownService, RpPairingFile},
    usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdDevice},
};
use std::time::Duration;

#[derive(Clone)]
pub struct Phone {
    pub device: UsbmuxdDevice,
    pub name: String,
}

pub async fn discover() -> Result<Vec<Phone>, String> {
    let mut mux = UsbmuxdAddr::default().connect(0).await.map_err(|_| {
        if cfg!(windows) { "Install or open Apple Devices from the Microsoft Store, then reconnect your iPhone." }
        else { "Connect your iPhone with a USB data cable. If it doesn't appear, open Finder and select it." }
    })?;
    let devices = mux
        .get_devices()
        .await
        .map_err(|_| "Couldn't check connected phones. Reconnect the USB cable.")?;
    let mut phones = Vec::new();
    for device in devices
        .into_iter()
        .filter(|d| d.connection_type == Connection::Usb)
    {
        let name = tokio::time::timeout(Duration::from_secs(1), async {
            let provider = device.to_provider(UsbmuxdAddr::default(), "Spoofed Helper");
            let mut lock = LockdownClient::new(provider.connect(62078).await?);
            lock.get_value(Some("DeviceName"), None).await
        })
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(|v| v.as_string().map(str::to_owned))
        .unwrap_or_else(|| "iPhone".into());
        phones.push(Phone { device, name });
    }
    phones.sort_by(|a, b| a.device.udid.cmp(&b.device.udid));
    Ok(phones)
}

fn device_error(e: IdeviceError) -> String {
    // Never format protocol responses: they can include private pairing material.
    match e {
        IdeviceError::UserDeniedPairing => "Trust was declined. Reconnect your iPhone and tap Trust.".into(),
        IdeviceError::PasswordProtected => "Unlock your iPhone with its passcode, then try again.".into(),
        _ => "The iPhone connection stopped. Keep it unlocked, approve Trust, and try again. If this repeats, open it in Finder or Apple Devices first.".into(),
    }
}

async fn service(
    provider: &UsbmuxdProvider,
    lock: &mut LockdownClient,
    name: &str,
    pair: &PairingFile,
) -> Result<Idevice, String> {
    let (port, ssl) = lock
        .start_service_with_escrow(name, pair.escrow_bag.clone())
        .await
        .map_err(device_error)?;
    let mut stream = provider.connect(port).await.map_err(device_error)?;
    if ssl {
        stream
            .start_session(pair, false)
            .await
            .map_err(device_error)?;
    }
    Ok(stream)
}

async fn read_small(afc: &mut AfcClient, path: &str) -> Result<Vec<u8>, String> {
    let mut file = afc
        .open(path, AfcFopenMode::RdOnly)
        .await
        .map_err(|_| "Open Spoofed → Setup → Receive from Computer, then try again.")?;
    let result = file.read_n(protocol::LIMIT + 1).await.map_err(device_error);
    let closed = file.close().await.map_err(device_error);
    let bytes = result?;
    closed?;
    Ok(bytes)
}

#[derive(Clone, Copy)]
pub enum Outcome {
    Accepted,
    Delivered,
}

pub async fn setup(phone: Phone, stage: impl Fn(&str) + Send) -> Result<Outcome, String> {
    stage("Unlock your iPhone and tap Trust if asked.");
    let provider = phone
        .device
        .to_provider(UsbmuxdAddr::default(), "Spoofed Helper");
    let mut mux = UsbmuxdAddr::default()
        .connect(0)
        .await
        .map_err(device_error)?;
    let mut lock = LockdownClient::new(provider.connect(62078).await.map_err(device_error)?);
    let pair = match mux.get_pair_record(&phone.device.udid).await {
        Ok(pair) => pair,
        Err(_) => {
            let buid = mux.get_buid().await.map_err(device_error)?;
            let mut pair = lock
                .pair(
                    uuid::Uuid::new_v4().to_string(),
                    buid,
                    Some("Spoofed Helper"),
                )
                .await
                .map_err(device_error)?;
            pair.udid = Some(phone.device.udid.clone());
            mux.save_pair_record(
                &phone.device.udid,
                pair.clone().serialize().map_err(device_error)?,
            )
            .await
            .map_err(device_error)?;
            pair
        }
    };
    lock.start_session(&pair).await.map_err(device_error)?;
    stage("Checking that Spoofed is ready…");
    let house = HouseArrestClient::new(
        service(&provider, &mut lock, "com.apple.mobile.house_arrest", &pair).await?,
    );
    let mut afc = house.vend_documents(protocol::APP_ID).await
        .map_err(|_| "Install the latest Spoofed app on this iPhone, open Setup → Receive from Computer, then try again.")?;
    let nonce = protocol::request_nonce(&read_small(&mut afc, protocol::REQUEST).await?)?;
    stage("Creating your pairing. Keep your iPhone unlocked…");
    lock.set_value(
        "EnableWifiDebugging",
        true.into(),
        Some("com.apple.mobile.wireless_lockdown"),
    )
    .await
    .map_err(device_error)?;
    let hostname = format!("Spoofed-{}", uuid::Uuid::new_v4());
    let mut rp = RpPairingFile::generate(&hostname);
    // Reconnect with the same credentials to validate and commit device-side trust.
    for _ in 0..2 {
        RemotePairingLockdownService::new(service(&provider, &mut lock, "com.apple.dt.remotepairingdeviced.lockdown", &pair).await
            .map_err(|_| "Couldn't open developer pairing. Enable Developer Mode in iPhone Settings → Privacy & Security, complete its restart, and try again.")?)
            .into_client(&hostname).map_err(device_error)?
            .connect(&mut rp, async || "000000".to_string()).await.map_err(device_error)?;
    }
    // The receive screen may have closed or restarted during the Trust prompt.
    if protocol::request_nonce(&read_small(&mut afc, protocol::REQUEST).await?)? != nonce {
        return Err(
            "The receive session changed. Keep Receive from Computer open and try again.".into(),
        );
    }
    stage("Sending pairing straight into Spoofed…");
    let bytes = protocol::envelope(&nonce, rp.to_bytes())?;
    let mut file = afc
        .open(protocol::PARTIAL, AfcFopenMode::WrOnly)
        .await
        .map_err(device_error)?;
    file.set_size(0).await.map_err(device_error)?;
    file.write_entire(&bytes).await.map_err(device_error)?;
    file.close().await.map_err(device_error)?;
    afc.rename(protocol::PARTIAL, protocol::DELIVERY)
        .await
        .map_err(device_error)?;
    stage("Sent. Waiting for Spoofed to confirm…");
    let confirmation = tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            if let Ok(bytes) = read_small(&mut afc, protocol::RECEIPT).await {
                match protocol::receipt(&bytes, &nonce) {
                    Some(true) => return Ok(Outcome::Accepted),
                    Some(false) => return Err("Spoofed couldn't accept the pairing. Check the message on your iPhone, then start receiving again.".into()),
                    None => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
    }).await.unwrap_or(Ok(Outcome::Delivered));
    let _ = tokio::time::timeout(Duration::from_secs(2), lock.stop_session()).await;
    confirmation
}
