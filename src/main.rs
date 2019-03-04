extern crate alsa;
extern crate libc;

use alsa::seq;
use std::error;
use std::ffi::CString;
use alsa::poll;
use std::boxed::Box;


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

fn print_midi_ev(ev: &seq::Event) -> Result<(), Box<error::Error>>{
    match ev.get_type() {
        seq::EventType::Noteon => {
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving Note On Event")?;
            println!("Note ON | {:?} | {:?} | {:?}", data.channel, data.note, data.velocity);
        }
        seq::EventType::Noteoff => {
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving Note On Event")?;
            println!("Note OFF | {:?} | {:?} | {:?}", data.channel, data.note, data.velocity);
        }
        _ => {
            ;
        }
    }

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

    loop {
        alsa::poll::poll(&mut fds, 1000)?;
        if input.event_input_pending(true)? == 0 {
            continue;
        }
        let ev = input.event_input()?;
        print_midi_ev(&ev)?;
    };

    Ok(())
}
