
use crate::{NewData, mpmcq, TEST_QUEUE_SIZE};
use std::{thread, time};
use std::sync::{Arc, Mutex, Condvar};
use crate::mpmcq::MyQueue;

pub fn run_queue_test<T:'static+NewData+Send+Sync>(nSender : i32, nReceiver : i32, durationS : i32)
                                                   -> Result<(), &'static str> {
    println!("========= test my fast queue: mpmcq ==========");

    let dataQ  = mpmcq::new_share_queue_capacity::<T>(TEST_QUEUE_SIZE);
    let resultSQ = mpmcq::new_share_queue::<i32>();
    let resultRQ = mpmcq::new_share_queue::<i32>();

    let condStart = Arc::new((Mutex::new(()), Condvar::new()));

    for i in 0..nReceiver {
        let readQ = dataQ.clone();
        let resultQ = resultRQ.clone();
        let condS = Arc::clone(&condStart);

        thread::spawn(move || {
            let id = i;
            println!("receiver {} start.", id);
            let (lock, cond) = &*condS;
            let g = lock.lock().unwrap();
            cond.wait(g);

            let begTime = time::Instant::now();
            let mut sum = 0;
            loop {
                let e = readQ.pop();
                match e {
                    Ok(x) => sum += 1,
                    Err(_) => break,
                }
                //if sum & 0xfffff == 0 {
                //    eprint!(".R");
                //}
            }
            let elapse = begTime.elapsed();
            resultQ.push(sum);
            println!("receiver {} end, receive {} times, elapse {} s, {} recv/s, {} ns/recv.",
                     id, sum, elapse.as_secs(), sum as f64 / elapse.as_secs() as f64,
                     elapse.as_nanos() / sum as u128);
        });
    }

    for i in 0..nSender {
        let writeQ = dataQ.clone();
        let resultQ = resultSQ.clone();
        let condS = Arc::clone(&condStart);

        thread::spawn(move || {
            let id = i;
            println!("sender {} start.", id);
            //let (lock, cond) = &*condS;
            //let g = lock.lock().unwrap();
            //cond.wait(g);
            let g = condS.0.lock().unwrap();
            condS.1.wait(g);

            let begTime = time::Instant::now();
            let mut sum = 0i32;
            loop {
                if let Err(_) = writeQ.push(T::new()) {
                    break;
                }
                sum += 1;
                //if sum & 0xfffff == 0 {
                //    eprint!(".S");
                //}
            }
            let elapse = begTime.elapsed();
            resultQ.push(sum);
            println!("sender {} end, send {} times, elapse {} s, {:.2} send/s, {} ns/send.",
                     id, sum, elapse.as_secs(), sum as f64 / elapse.as_secs() as f64,
                     elapse.as_nanos() / sum as u128);
        });
    }

    thread::sleep(time::Duration::from_secs(2));
    condStart.1.notify_all();

    let begTime = time::Instant::now();
    thread::sleep(time::Duration::from_secs(durationS as u64));
    dataQ.close_send();

    let mut sumRecv = 0;
    for i in 0..nReceiver {
        let sum = resultRQ.pop().unwrap_or(0);
        sumRecv += sum;
    }

    let mut sumSend = 0;
    for i in 0..nSender {
        let sum = resultSQ.pop().unwrap_or(0);
        sumSend += sum;
    }

    let elapse = begTime.elapsed();
    thread::sleep(time::Duration::from_secs(1));

    println!("total send {}, recv {}, cost {} s, data size is {} bytes.",
             sumSend, sumRecv, elapse.as_secs(), std::mem::size_of::<T>());
    println!("  {} send/s, {} recv/s", sumSend as f64 / elapse.as_secs() as f64,
             sumRecv as f64 / elapse.as_secs() as f64,);
    println!("  {} ns/send, {} ns/recv", elapse.as_nanos()/ sumSend as u128,
             elapse.as_nanos()/ sumRecv as u128);

    Ok(())
}