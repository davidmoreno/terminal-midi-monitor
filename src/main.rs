#[macro_use]
extern crate lazy_static;
extern crate clap;
extern crate alsa;
extern crate libc;

use alsa::seq;
use std::error;
use std::ffi::CString;
use colored::*;
use std::collections::HashMap;
use std::time::Instant;
use clap::{Arg, App};
use std::io;
use std::io::prelude::*;

lazy_static! {
    static ref CC_MAP: HashMap<u32, String> = build_cc_map();
}
const BPM_DAMPING: f64 = 0.03;

struct MidiMonitor<'a> {
    start_time: Instant,
    seq: &'a seq::Seq,
    last_clock: f64,
    average_sec_per_clock: f64,  // Rolling average
    clock_pos: i32, // Song position. once per clock.
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


fn setup_alsaseq() -> Result<(seq::Seq, i32), Box<error::Error>>{
    let seq = seq::Seq::open(None, Some(alsa::Direction::Capture), true)?;
    seq.set_client_name(&CString::new("Terminal MIDI Monitor")?)?;

    let mut dinfo = seq::PortInfo::empty()?;
    dinfo.set_capability(seq::WRITE | seq::SUBS_WRITE);
    dinfo.set_type(seq::MIDI_GENERIC | seq::APPLICATION);
    dinfo.set_name(&CString::new("Input")?);
    seq.create_port(&dinfo)?;

    let input_port = dinfo.get_port();

    Ok((seq, input_port))
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

fn get_origin(midi_monitor: &MidiMonitor, ev: &seq::Event) -> Result<String, Box<error::Error>> {
    let source = ev.get_source();
    let origin = format!("{}:{}",
        midi_monitor.seq.get_any_client_info(source.client)?.get_name()?,
        midi_monitor.seq.get_any_port_info(source)?.get_name()?,
    );

    Ok(origin)
}

fn print_midi_ev(midi_monitor: &mut MidiMonitor, ev: &seq::Event) -> Result<(), Box<error::Error>>{
    let elapsed = midi_monitor.start_time.elapsed();
    let elapsed: f64 = elapsed.as_secs() as f64 + elapsed.subsec_millis() as f64 / 1000.0;
    let mut event;
    let mut extra_data: String = "".to_string();
    let origin = get_origin(&midi_monitor, &ev)?;

    match ev.get_type() {
        seq::EventType::Noteon => {
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving event data")?;
            event = if data.velocity > 0 {
                "Note ON ".green()
            } else {
                "Note ON ".red()
            };
            extra_data = format!(
                "Channel {:2} | {:<3} ({}) | {}",
                data.channel.to_string().white().dimmed(),
                note_name(data.note),
                data.note,
                data.velocity
            );
        },
        seq::EventType::Noteoff => {
            event = "Note OFF".red();
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving event data")?;
            extra_data = format!(
                "Channel {:2} | {:<3} ({}) | {}",
                data.channel.to_string().white().dimmed(),
                note_name(data.note),
                data.note,
                data.velocity
            );
        },
        seq::EventType::Controller => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Controller Change".blue();
            extra_data = format!(
                "Channel {:2} | CC {:3} | {:3} | {} ",
                data.channel,
                data.param,
                data.value,
                CC_MAP.get(&data.param).unwrap_or(&"Unknown".to_string()),
            );
        },
        seq::EventType::Pitchbend => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Pitch Bend".purple();
            extra_data = format!(
                "Channel {:2} | {} ",
                data.channel,
                data.value,
            );
        },
        seq::EventType::Pgmchange => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Program Change".purple();
            extra_data = format!(
                "{}",
                data.value,
            );
        },
        seq::EventType::Chanpress => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Channel Pressure".purple();
            extra_data = format!(
                "Channel {:2} | {}",
                data.channel,
                data.value,
            );
        }
        seq::EventType::Clock => {
            midi_monitor.clock_pos += 1;
            midi_monitor.average_sec_per_clock =
                ((elapsed - midi_monitor.last_clock) * BPM_DAMPING) +
                midi_monitor.average_sec_per_clock as f64 * (1.0 - BPM_DAMPING);
            midi_monitor.last_clock = elapsed;

            // I hope RUST simplifies this.. as I prefer clean code.
            let cs = 1.0 / midi_monitor.average_sec_per_clock;
            let bs = cs / 24.0; // 24 clocks per beat -> beats per second
            let bpm = bs * 60.0;

            // Show only once per beat
            if midi_monitor.clock_pos % 24 == 0 {
                print!(
                    "{:10.3} | {:20} | {:>17} | {:>3.1} BPM | Clock Position {}               \r",
                    elapsed, origin, "Clock".purple(), bpm, midi_monitor.clock_pos
                );
                io::stdout().flush()?;
            }
            return Ok(());
        }
        seq::EventType::Start => {
            event = "Start".purple();
            midi_monitor.clock_pos = 0;
        }
        seq::EventType::Stop => {
            event = "Stop".purple();
        }
        seq::EventType::Continue => {
            event = "Continue".purple();
        }
        _ => {
            event = format!("{:?}", ev).cyan();
        }
    }
    println!(
        "{:10.3} | {:20} | {:>17} | {}                                          ",
        elapsed,
        origin,
        event,
        extra_data
    );

    Ok(())
}

fn autoconnect_all(seq: &alsa::seq::Seq, port: i32) -> Result<(), Box<error::Error>> {
    for from_info in seq::ClientIter::new(&seq){
        for from_port in seq::PortIter::new(&seq, from_info.get_client()){
            if from_port.get_capability().contains(seq::SUBS_READ) && !from_port.get_capability().contains(seq::NO_EXPORT){
                let subs = seq::PortSubscribe::empty()?;
                let sender = seq::Addr{ client: from_port.get_client(), port: from_port.get_port() };
                subs.set_sender(sender);
                subs.set_dest(seq::Addr{ client: seq.client_id()?, port: port });
                match seq.subscribe_port(&subs) {
                    Ok(_) => {
                        println!("{} {}",
                            "Connected and receiving data from".green(),
                            format!("{}:{}",
                                from_port.get_name()?,
                                seq.get_any_port_info(sender)?.get_name()?.to_string()
                            ).blue()
                        );
                    },
                    Err(err) =>
                        println!("ERROR: {:?}", err)
                }
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<error::Error>> {
    println!("Terminal MIDI Monitor. (C) 2019 Coralbits SL. Licensed under GPL v3.");
    let matches = App::new("Terminal MIDI Monitor")
        .version("0.1.0")
        .author("David Moreno <dmoreno@coralbits.com>")
        .about("Terminal monitor for Alsa Seq MIDI events.")
        .arg(
            Arg::with_name("autoconnect")
                .short("a")
                .long("autoconnect")
                .help("Autoconnects all outputs to the monitor. Also new clients are automatically connected.")
            )
        .get_matches();
    let (seq, port) = setup_alsaseq()?;

    let autoconnect = matches.occurrences_of("autoconnect") > 0;
    if autoconnect {
        println!("{}", "Autoconnect ON".yellow());
        autoconnect_all(&seq, port)?;
    }

    let mut input = seq.input();

    println!("Waiting for connections.");

    use alsa::PollDescriptors;
    let seqp = (&seq, Some(alsa::Direction::Capture));
    let mut fds = Vec::<libc::pollfd>::new();
    fds.append(&mut seqp.get()?);

    let mut midi_monitor = MidiMonitor{
        start_time: Instant::now(),
        seq: &seq,
        average_sec_per_clock: (60.0 / 120.0) / 24.0,
        last_clock: 0.0,
        clock_pos: 0,
    };

    loop {
        alsa::poll::poll(&mut fds, 1000)?;
        if input.event_input_pending(true)? == 0 {
            continue;
        }
        let ev = input.event_input()?;

        match print_midi_ev(&mut midi_monitor, &ev) {
            Ok(()) => {

            },
            err => {
                println!("{}", format!("ERROR: {:?}",err).red());
            }
        };
    };

    Ok(())
}
