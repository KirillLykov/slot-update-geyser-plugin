use {
    agave_geyser_plugin_interface::geyser_plugin_interface::SlotStatus as GeyserSlotStatus,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    solana_clock::Slot,
    solana_time_utils::timestamp,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotMessage {
    pub slot: Slot,
    pub parent: Option<Slot>,
    #[serde(with = "slot_status_as_str")]
    pub status: GeyserSlotStatus,
    pub dead_error: Option<String>,
    pub created_at: u64,
}

impl SlotMessage {
    pub fn from_geyser(slot: Slot, parent: Option<Slot>, status: &GeyserSlotStatus) -> Self {
        Self {
            slot,
            parent,
            status: status.clone(),
            dead_error: if let GeyserSlotStatus::Dead(error) = status {
                Some(error.clone())
            } else {
                None
            },
            created_at: timestamp(),
        }
    }
}

pub mod slot_status_as_str {
    use super::*;
    use agave_geyser_plugin_interface::geyser_plugin_interface::SlotStatus;

    pub fn serialize<S>(status: &SlotStatus, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(status.as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SlotStatus, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "confirmed" => Ok(SlotStatus::Confirmed),
            "processed" => Ok(SlotStatus::Processed),
            "rooted" => Ok(SlotStatus::Rooted),
            "first_shred_received" => Ok(SlotStatus::FirstShredReceived),
            "completed" => Ok(SlotStatus::Completed),
            "created_bank" => Ok(SlotStatus::CreatedBank),
            "dead" => Ok(SlotStatus::Dead("dead".to_string())),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &[
                    "processed",
                    "rooted",
                    "confirmed",
                    "first_shred_received",
                    "completed",
                    "created_bank",
                    "dead",
                ],
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::message::SlotMessage,
        agave_geyser_plugin_interface::geyser_plugin_interface::SlotStatus,
        solana_time_utils::timestamp,
    };

    #[test]
    fn test_from_geyser_with_non_dead_status() {
        let status = SlotStatus::Processed;
        let expected = SlotMessage {
            slot: 12345,
            parent: Some(12344),
            status: status.clone(),
            dead_error: None,
            created_at: timestamp(),
        };

        let actual = SlotMessage::from_geyser(12345, Some(12344), &status);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_slot_message_serialization_roundtrip() {
        let msg = SlotMessage {
            slot: 77,
            parent: Some(70),
            status: SlotStatus::Completed,
            dead_error: None,
            created_at: timestamp(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""completed""#));

        let decoded: SlotMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, msg);
    }
}
