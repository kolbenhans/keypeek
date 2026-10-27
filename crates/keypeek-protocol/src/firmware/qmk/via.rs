use super::common::{EncoderSource, QmkFeatures, QmkProtocol};
use crate::protocols::DeviceError;
use qmk_via_api::api::KeyboardApi;

pub struct ViaProtocol;

impl ViaProtocol {
    pub fn connect(vid: u16, pid: u16, json_path: &str) -> Result<QmkProtocol, DeviceError> {
        let definition = super::catalog::load_desktop_via_definition(vid, pid, json_path)?;
        let api = Self::get_api(definition.vid, definition.pid)?;
        let features = QmkFeatures::probe(&api);

        Ok(QmkProtocol::new(
            api,
            definition,
            features,
            EncoderSource::Via,
        ))
    }

    fn get_api(vid: u16, pid: u16) -> Result<KeyboardApi, DeviceError> {
        // A read timeout keeps command/response round trips bounded so the HID
        // reader loop stays responsive between commands.
        let api = KeyboardApi::new(vid, pid, 0xff60, Some(250)).map_err(|e| {
            DeviceError::Transport(format!(
                "Failed to connect to device ({vid:04x}:{pid:04x}): {e}"
            ))
        })?;

        let protocol_version = api
            .get_protocol_version()
            .map_err(|e| DeviceError::Protocol(format!("Failed to get protocol version: {e}")))?;

        if protocol_version < 12 {
            return Err(DeviceError::Unsupported(format!(
                "Unsupported protocol version: {protocol_version}. Minimum required version is 12."
            )));
        }

        Ok(api)
    }
}
