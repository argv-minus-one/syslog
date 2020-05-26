extern crate slog;
extern crate slog_syslog;
extern crate syslog;

use slog::*;
use slog_syslog::*;
use std::net::UdpSocket;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

#[test]
fn integration_test() {
    let server_socket = UdpSocket::bind("localhost:0").expect("couldn't bind server socket");
    let server_addr = server_socket.local_addr().expect("couldn't get server socket address");

    // Start a server.
    let server_thread = thread::spawn(move || {
        server_socket.set_read_timeout(Some(Duration::from_secs(10))).expect("couldn't set server socket read timeout");

        let mut packets = Vec::<Box<[u8]>>::new();
        let mut buf = [0u8; 65535];

        loop {
            let (pkt_size, _) = server_socket.recv_from(&mut buf).expect("server couldn't receive packet");

            if pkt_size == 4 && &buf[0..4] == b"STOP" {
                break;
            }

            packets.push(Box::from(&buf[..pkt_size]));
        }

        packets
    });

    let client_addr = {
        let mut client_addr = server_addr.clone();
        client_addr.set_port(0);
        client_addr
    };

    {
        // Set up a logger.
        let logger = Logger::root_typed(
            Mutex::new(Streamer3164::new(syslog::udp(
                syslog::Formatter3164 {
                    facility: Facility::LOG_USER,
                    hostname: Some("test-hostname".to_string()),
                    process: "test-app".to_string(),
                    pid: 123
                },
                &client_addr,
                &server_addr
            ).expect("couldn't create syslog logger"))).fuse(),
            o!("key" => "value")
        );

        // Log a test message.
        info!(logger, "Hello, world!"; "key2" => "value2");
    }

    {
        // Tell the server thread to stop.
        let client_socket = UdpSocket::bind(client_addr).expect("couldn't bind client socket");
        client_socket.send_to(b"STOP", &server_addr).expect("couldn't send stop packet");
    }

    // Get the logs received by the server thread.
    let logs = server_thread.join().expect("server thread panicked");

    // Check that the logs were correct.
    assert_eq!(logs.len(), 1);

    let s = String::from_utf8(logs[0].to_vec()).expect("log packet contains invalid UTF-8");
    assert!(s.starts_with("<14>"));
    assert!(s.ends_with("test-hostname test-app[123]: Hello, world! [key=\"value\" key2=\"value2\"]"));
}
