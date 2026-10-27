//! Hardware transport interface for USB and BLE HID devices.

use super::DeviceError;
use std::time::Duration;

#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::{Arc, Mutex};

/// Interface for raw HID device endpoints.
pub trait RawHidTransport: Send {
    /// Writes raw output report bytes to the HID device.
    fn write_output_report(&mut self, data: &[u8]) -> Result<(), DeviceError>;

    /// Reads an incoming input report with the specified timeout.
    fn read_input_report(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError>;
}

/// In-memory mock transport for testing.
#[cfg(test)]
#[derive(Default, Debug, Clone)]
pub struct MockHidTransport {
    incoming: Arc<Mutex<VecDeque<Vec<u8>>>>,
    written: Arc<Mutex<Vec<Vec<u8>>>>,
}

#[cfg(test)]
impl MockHidTransport {
    /// Creates a new, empty mock transport.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues an incoming raw packet that will be returned by [`RawHidTransport::read_input_report`].
    pub fn push_incoming(&self, packet: Vec<u8>) {
        self.incoming.lock().unwrap().push_back(packet);
    }

    /// Returns a snapshot of all raw packets written via [`RawHidTransport::write_output_report`].
    pub fn written_packets(&self) -> Vec<Vec<u8>> {
        self.written.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl RawHidTransport for MockHidTransport {
    fn write_output_report(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        self.written.lock().unwrap().push(data.to_vec());
        Ok(())
    }

    fn read_input_report(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError> {
        Ok(self.incoming.lock().unwrap().pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::{pump_hid_reader, DeviceEvent, EventSubscription, StopSignal};
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn mock_transport_write_and_read() {
        let mut transport = MockHidTransport::new();
        transport.push_incoming(vec![1, 2, 3]);

        let read = transport
            .read_input_report(Duration::from_millis(10))
            .unwrap();
        assert_eq!(read, Some(vec![1, 2, 3]));

        let empty = transport
            .read_input_report(Duration::from_millis(10))
            .unwrap();
        assert_eq!(empty, None);

        transport.write_output_report(&[0xC0, 0xA1]).unwrap();
        assert_eq!(transport.written_packets(), vec![vec![0xC0, 0xA1]]);
    }

    #[test]
    fn mock_transport_shares_state_on_clone() {
        let transport = MockHidTransport::new();
        let mut clone = transport.clone();

        clone.write_output_report(&[42]).unwrap();
        assert_eq!(transport.written_packets(), vec![vec![42]]);

        transport.push_incoming(vec![99]);
        assert_eq!(
            clone.read_input_report(Duration::from_millis(10)).unwrap(),
            Some(vec![99])
        );
    }

    #[test]
    fn mock_transport_pumps_events_through_reader() {
        let transport = MockHidTransport::new();
        let mut reader_transport = transport.clone();

        // Queue a key event packet: [0xF1, row=1, col=2, pressed=1]
        transport.push_incoming(vec![0xF1, 1, 2, 1]);

        let (tx, rx) = mpsc::channel();
        let stop_signal = StopSignal::new();
        let handle = thread::spawn(move || {
            pump_hid_reader(
                || {
                    match reader_transport.read_input_report(Duration::from_millis(10)) {
                        Ok(Some(bytes)) => Ok(Some(bytes)),
                        Ok(None) => Err("disconnect".to_string()), // trigger exit after first read
                        Err(e) => Err(e.to_string()),
                    }
                },
                tx,
                "Disconnected",
                stop_signal,
            );
        });

        let event = rx.recv().unwrap();
        assert_eq!(
            event,
            DeviceEvent::KeyPressed {
                row: 1,
                col: 2,
                pressed: true
            }
        );

        let disconnect = rx.recv().unwrap();
        assert_eq!(
            disconnect,
            DeviceEvent::Disconnected("Disconnected".to_string())
        );
        let _ = handle.join();
    }

    #[test]
    fn mock_transport_pump_reader_stops_on_signal() {
        let (tx, rx) = mpsc::channel();
        let stop_signal = StopSignal::new();
        let stop_clone = stop_signal.clone();

        let handle = thread::spawn(move || {
            pump_hid_reader(|| Ok(None), tx, "Disconnected", stop_clone);
        });

        // Set stop signal to true
        stop_signal.stop();
        let _ = handle.join();

        // Channel should close without emitting any Disconnected event
        assert!(rx.recv().is_err());
    }

    #[test]
    fn test_pump_reader_exits_when_subscription_dropped_on_inactive_device() {
        let (tx, rx) = mpsc::channel();
        let stop_signal = StopSignal::new();
        let subscription = EventSubscription::new(rx, stop_signal.clone());

        let handle = thread::spawn(move || {
            // Inactive device that returns timeout Ok(None)
            pump_hid_reader(|| Ok(None), tx, "Disconnected", stop_signal);
        });

        // Drop the RAII subscription guard without manually stopping stop_signal
        drop(subscription);

        // The reader thread must detect the stop signal triggered on drop and exit cleanly
        let join_result = handle.join();
        assert!(join_result.is_ok());
    }

    #[test]
    fn test_pump_reader_exits_when_subscription_dropped_with_unmatched_packets() {
        let (tx, rx) = mpsc::channel();
        let stop_signal = StopSignal::new();
        let subscription = EventSubscription::new(rx, stop_signal.clone());

        let handle = thread::spawn(move || {
            // Device returns raw packets that don't match KeyPeek telemetry
            pump_hid_reader(
                || Ok(Some(vec![0x00, 0x12, 0x34])),
                tx,
                "Disconnected",
                stop_signal,
            );
        });

        // Drop the RAII subscription guard without manually stopping stop_signal
        drop(subscription);

        // The reader thread must detect the stop signal triggered on drop and exit cleanly
        let join_result = handle.join();
        assert!(join_result.is_ok());
    }
}
