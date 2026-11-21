use super::*;

#[test]
fn the_recorded_views_agent_gets_the_dwell_and_the_way() {
    let config = RecorderConfig::default();
    assert_eq!(
        wishes(&config),
        AgentRecord {
            menu_hovers: true,
            hover_dwell_ms: 250,
            record_pointer: AgentRecordPointer::Menu,
        }
    );
    for (hover, menu_hovers) in [
        (RecordHover::Menu, true),
        (RecordHover::All, true),
        (RecordHover::Off, false),
    ] {
        for (path, sent) in [
            (RecordPointer::Menu, AgentRecordPointer::Menu),
            (RecordPointer::All, AgentRecordPointer::All),
            (RecordPointer::Off, AgentRecordPointer::Off),
        ] {
            let config = RecorderConfig {
                record_hover: hover,
                hover_dwell_ms: 600,
                record_pointer: path,
                ..RecorderConfig::default()
            };
            assert_eq!(
                wishes(&config),
                AgentRecord {
                    menu_hovers,
                    hover_dwell_ms: 600,
                    record_pointer: sent,
                }
            );
        }
    }
}
