use clap::Parser;
use nix::poll::{ppoll, PollFd, PollFlags};
use nix::sys::time::TimeSpec;
use nix::sys::time::TimeValLike;
use serialport::TTYPort;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::os::fd::{AsFd, BorrowedFd};
use std::thread;

#[derive(Parser, Clone)]
#[command(version, about, long_about = None)]
struct Cli {
    /// the port to use with the address
    #[arg(short, long, default_value_t = 2024.to_string())]
    port: String,

    /// the ip address
    #[arg(short, long, required = false, default_value = "0.0.0.0")]
    address: String,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    /// Serial Device to connect to
    #[arg(short, long, default_value = "/dev/ttyACM0")]
    serial_dev: String,

    /// baud to use with the serial_dev
    #[arg(short, long, default_value_t = 460800)]
    baud: u32,
}

fn main() {
    let cli = Cli::parse();

    // You can see how many times a particular flag or argument occurred
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => println!("Debug mode is off"),
        1 => println!("Debug mode is kind of on"),
        2 => println!("Debug mode is on"),
        _ => println!("Debug mode is ON"),
    }

    println!("port: {}", cli.port);
    println!("serial_dev: {}", cli.serial_dev);
    println!("baud: {}", cli.baud);

    let serialport = cli.serial_dev.clone();
    let hostname = cli.address.to_owned() + ":" + &cli.port.to_owned();

    println!("Bind to address: {}", hostname);
    let listener = TcpListener::bind(hostname).unwrap();

    loop {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let read_stream = stream.try_clone().expect("failed to clone stream");
                    let serialport = serialport.clone();
                    thread::spawn(move || {
                        let _ = handle_connection(read_stream, &serialport, cli.baud, cli.debug);
                    });
                }
                Err(e) => {
                    println!("connection failed: {}", e);
                }
            }
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    serial_dev: &str,
    baud: u32,
    debug: u8,
) -> Result<(), std::io::Error> {
    let immut_stream = stream
        .try_clone()
        .expect("failed to clone immutable stream");
    if debug >= 1 {
        println!("enter handle_connection()");
    }
    let mut ser_port = TTYPort::open(&serialport::new(serial_dev, baud)).expect("port in use...");

    let mut stream_fds_in = [PollFd::new(immut_stream.as_fd(), PollFlags::POLLIN)];
    let timeout = TimeSpec::milliseconds(1);
    let mut serial_fds;
    unsafe {
        let borrowed_fd = BorrowedFd::borrow_raw(ser_port.as_raw_fd());
        serial_fds = [PollFd::new(borrowed_fd, PollFlags::POLLIN)];
    }

    let mut rd_buf = [0; 4096];
    let mut stream_sig: bool = false;
    let mut stream_sig_brkn_pipe: bool = false;
    let mut serial_sig: bool = false;
    let mut serial_sig_err: bool = false;

    loop {
        let nfds_stream: i16 = ppoll(&mut stream_fds_in, Some(timeout), None)
            .unwrap()
            .try_into()
            .unwrap();
        let nfds_serial: i16 = ppoll(&mut serial_fds, Some(timeout), None)
            .unwrap()
            .try_into()
            .unwrap();

        if nfds_stream >= 0 {
            stream_sig = stream_fds_in[0]
                .revents()
                .unwrap()
                .contains(PollFlags::POLLIN);
        }
        if nfds_stream >= 0 {
            stream_sig_brkn_pipe = stream_fds_in[0]
                .revents()
                .unwrap()
                .contains(PollFlags::POLLERR | PollFlags::POLLHUP);
        }
        if nfds_serial >= 0 {
            serial_sig = serial_fds[0].revents().unwrap().contains(PollFlags::POLLIN);
        }
        if nfds_serial >= 0 {
            serial_sig_err = serial_fds[0]
                .revents()
                .unwrap()
                .contains(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL);
        }
        if stream_sig_brkn_pipe {
            println!("stream: broken pipe received, returning to main loop");
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stream: broken pipe received, returning to main loop",
            ));
        }
        if serial_sig_err {
            println!("serial: err received, returning to main loop");
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "serial: err received, returning to main loop",
            ));
        }
        if stream_sig {
            let rd_st = stream.read(&mut rd_buf).unwrap();
            if rd_st > 0 {
                if debug >= 2 {
                    println!("got {} bytes of data from stream", rd_st);
                }
                let cp_status = ser_port.write(&rd_buf[0..rd_st]);
                match cp_status {
                    Ok(cp_status) => {
                        if debug >= 2 {
                            println!("serial port write status {}", cp_status);
                        }
                    }
                    Err(e) => {
                        println!("error writing to serialport. {}", e);
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::NotConnected,
                            "notconnected received, returning to main loop",
                        ));
                    }
                }
            }
            stream_sig = false;
        }
        if serial_sig {
            let cp_read_status = ser_port.read(&mut rd_buf);
            match cp_read_status {
                Ok(cp_bytes_read) => {
                    if cp_bytes_read > 0 {
                        if debug >= 2 {
                            println!("got {} bytes of data from serialport", cp_bytes_read);
                        }
                        let wr_status = stream.write(&rd_buf[0..cp_bytes_read]);
                        match wr_status {
                            Ok(cp_status) => {
                                if debug >= 2 {
                                    println!("stream write status {}", cp_status);
                                }
                            }
                            Err(e) => {
                                println!("error writing to stream. {}. returning to main loop", e);
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::BrokenPipe,
                                    "broken pipe received, returning to main loop",
                                ));
                            }
                        }
                    }
                }
                Err(_e) => {
                    println!("error reading from serialport. {}", _e);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "notconnected received, returning to main loop",
                    ));
                }
            }
            serial_sig = false;
        }
    }
}
