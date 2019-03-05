#[macro_use]
extern crate lazy_static;

extern crate alsa;
extern crate libc;

use alsa::seq;
use std::error;
use std::ffi::CString;
use colored::*;
use std::collections::HashMap;
use std::time::Instant;


lazy_static! {
    static ref CC_MAP: HashMap<u32, String> = build_cc_map();
}


// List from http://nickfever.com/music/midi-cc-list
fn build_cc_map() -> HashMap<u32, String> {
    [
        (0, "Bank Select".to_string()),
        (1, "Modulation".to_string()),
        (2, "Breath Controller".to_string()),
        (3, "Undefined".to_string()),
        (4, "Foot Controller".to_string()),
        (5, "Portamento Time".to_string()),
        (6, "Data Entry Most Significant Bit(MSB)".to_string()),
        (7, "Volume".to_string()),
        (8, "Balance".to_string()),
        (9, "Undefined".to_string()),
        (10, "Pan".to_string()),
        (11, "Expression".to_string()),
        (12, "Effect Controller 1".to_string()),
        (13, "Effect Controller 2".to_string()),
        // (14, "Undefined".to_string()),
        // (15, "Undefined".to_string()),
        //(1, "General Purpose".to_string()),
        //(1, "Undefined".to_string()),
        // (1, "Controller 0-31 Least Significant Bit (LSB)".to_string()),
        (64, "Damper Pedal / Sustain Pedal".to_string()),
        (65, "Portamento On/Off Switch".to_string()),
        (66, "Sostenuto On/Off Switch".to_string()),
        (67, "Soft Pedal On/Off Switch".to_string()),
        (68, "Legato FootSwitch".to_string()),
        (69, "Hold 2".to_string()),
        (70, "Sound Controller 1".to_string()),
        (71, "Sound Controller 2".to_string()),
        (72, "Sound Controller 3".to_string()),
        (73, "Sound Controller 4".to_string()),
        (74, "Sound Controller 5".to_string()),
        (75, "Sound Controller 6".to_string()),
        (76, "Sound Controller 7".to_string()),
        (77, "Sound Controller 8".to_string()),
        (78, "Sound Controller 9".to_string()),
        (79, "Sound Controller 10".to_string()),
        (80, "General Purpose MIDI CC Controller".to_string()),
        (81, "General Purpose MIDI CC Controller".to_string()),
        (82, "General Purpose MIDI CC Controller".to_string()),
        (83, "General Purpose MIDI CC Controller".to_string()),
        (84, "Portamento CC Control".to_string()),
        // (, "Undefined".to_string()),
        (91, "Effect 1 Depth".to_string()),
        (92, "Effect 2 Depth".to_string()),
        (93, "Effect 3 Depth".to_string()),
        (94, "Effect 4 Depth".to_string()),
        (95, "Effect 5 Depth".to_string()),
        (96, "(+1) Data Increment".to_string()),
        (97, "(-1) Data Decrement".to_string()),
        (98, "Non-Registered Parameter Number LSB (NRPN)".to_string()),
        (99, "Non-Registered Parameter Number MSB (NRPN)".to_string()),
        (100, "Registered Parameter Number LSB (RPN)".to_string()),
        (101, "Registered Parameter Number MSB (RPN)".to_string()),
        // (1, "Undefined".to_string()),
        // (1, "".to_string()),
        (120, "All Sound Off".to_string()),
        (121, "Reset All Controllers".to_string()),
        (122, "Local On/Off Switch".to_string()),
        (123, "All Notes Off".to_string()),
        (124, "Omni Mode Off".to_string()),
        (125, "Omni Mode On".to_string()),
        (126, "Mono Mode".to_string()),
        (127, "Poly Mode".to_string()),
    ].iter().cloned().collect()
}


fn setup_alsaseq() -> Result<seq::Seq, Box<error::Error>>{
    let seq = seq::Seq::open(None, Some(alsa::Direction::Capture), true)?;
    seq.set_client_name(&CString::new("Terminal MIDI Monitor")?)?;

    let mut dinfo = seq::PortInfo::empty()?;
    dinfo.set_capability(seq::WRITE | seq::SUBS_WRITE);
    dinfo.set_type(seq::MIDI_GENERIC | seq::APPLICATION);
    dinfo.set_name(&CString::new("Input")?);
    seq.create_port(&dinfo)?;

    // let input_port = dinfo.get_port();

    Ok(seq)
}

fn note_name(note: u8) -> String {
    let note_name = match note % 12 {
        0 => "C",
        1 => "C#",
        2 => "D",
        3 => "D#",
        4 => "E",
        5 => "F",
        6 => "F#",
        7 => "G",
        8 => "G#",
        9 => "A",
        10 => "A#",
        11 => "B",
        _ => "??"
    };
    format!("{}{}", note_name, note / 12)
}

fn print_midi_ev(now: &Instant, ev: &seq::Event) -> Result<(), Box<error::Error>>{
    let elapsed = now.elapsed();
    let elapsed = elapsed.as_secs() as f32 + elapsed.subsec_millis() as f32 / 1000.0;
    let mut event: ColoredString = format!("{:?}", ev.get_type()).red();
    let mut extra_data: String = "".to_string();

    match ev.get_type() {
        seq::EventType::Noteon => {
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving Note On Event")?;
            event = if data.velocity > 0 {
                "Note ON ".green()
            } else {
                "Note ON ".red()
            };
            extra_data = format!(
                "Channel {} | {:<3} ({}) | {}",
                data.channel.to_string().white().dimmed(),
                note_name(data.note),
                data.note,
                data.velocity
            );
        },
        seq::EventType::Noteoff => {
            event = "Note OFF".red();
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving Note On Event")?;
            extra_data = format!(
                "Channel {} | {:<3} ({}) | {}",
                data.channel.to_string().white().dimmed(),
                note_name(data.note),
                data.note,
                data.velocity
            );
        },
        seq::EventType::Controller => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving Note On Event")?;
            event = "Controller Change".blue();
            extra_data = format!(
                "Channel {} | CC {:3} | {:3} | {} ",
                data.channel,
                data.param,
                data.value,
                CC_MAP.get(&data.param).unwrap_or(&"Unknown".to_string()),
            );
        },
        _ => {

        }
    }
    println!(
        "{:10.3} | {:>20} | {}",
        elapsed,
        event,
        extra_data
    );

    Ok(())
}

fn main() -> Result<(), Box<error::Error>> {
    println!("Terminal MIDI Monitor. (C) 2019 Coralbits SL. Licensed under GPL v3.");

    let seq = setup_alsaseq()?;
    let mut input = seq.input();

    println!("Waiting for connections.");

    use alsa::PollDescriptors;
    let seqp = (&seq, Some(alsa::Direction::Capture));
    let mut fds = Vec::<libc::pollfd>::new();
    fds.append(&mut seqp.get()?);

    let now = Instant::now();

    loop {
        alsa::poll::poll(&mut fds, 1000)?;
        if input.event_input_pending(true)? == 0 {
            continue;
        }
        let ev = input.event_input()?;
        print_midi_ev(&now, &ev)?;
    };

    Ok(())
}
