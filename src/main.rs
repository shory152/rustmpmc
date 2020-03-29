use std::{process, env};
use mpmc::{QueueConf, run_queue_test};

fn usage() {
    let mut args = env::args();
    println!("Usage: {} dataSize nSender nReceiver testDuration queueType", args.next().unwrap());
    println!("  queueType: MYFQ | MYMPMC1 | MYMPMC2 | MPSC | MPSCQ");
}

fn main() {
    let qconf = QueueConf::new(env::args()).unwrap_or_else(|err|{
        eprintln!("Parse args error. {}", err);
        usage();
        process::exit(1);
    });

    if let Err(err) = run_queue_test(qconf) {
        eprintln!("Run test error. {}", err);
        process::exit(1);
    }
}
