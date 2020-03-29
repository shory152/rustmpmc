//! test my queue
//!
//!
//!

pub mod dump;
mod mysync;
mod FastQueue;
mod testfq;
mod testmpsc;
mod testmpscq;
mod mpmcq;
mod mpmcq2;
mod testmpmcq;
mod testmpmcq2;

use std::sync::{Arc, Mutex, Condvar};
use FastQueue::FQueue;
use std::{thread, time};
use crate::QueueType::*;

#[derive(PartialOrd, PartialEq)]
pub enum QueueType {
    MyFastQueue,
    MyMPMC1,
    MyMPMC2,
    StdMpsc,
    StdMpscQ,
}

pub struct QueueConf {
    queueType : QueueType,
    dataSize : i32,
    nSender : i32,
    nReceiver : i32,
    duration : i32, // seconds
}
impl QueueConf {
    pub fn new(mut args: std::env::Args) -> Result<QueueConf, &'static str> {
        args.next(); // skip program name

        // parse data size
        let dataSizeStr = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a 'data size'"),
        };
        let dataSize = match dataSizeStr.parse::<i32>() {
            Ok(v) => v,
            Err(_) => return Err("Invalid args. Give a integer for data size"),
        };

        // parse sender count
        let nSenderStr = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a 'sender count'"),
        };
        let nSender = match nSenderStr.parse::<i32>(){
            Ok(v) => v,
            Err(_) => return Err("Invalid args. Give a integer for sender count"),
        };

        // parse receiver count
        let nReceiverStr = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a 'sender count'"),
        };
        let nReceiver = match nReceiverStr.parse::<i32>(){
            Ok(v) => v,
            Err(_) => return Err("Invalid args. Give a integer for receiver count"),
        };

        // parse test duration in seconds
        let durationStr = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a 'test duration in seconds'"),
        };
        let duration = match durationStr.parse::<i32>(){
            Ok(v) => v,
            Err(_) => return Err("Invalid args. Give a integer for duration in seconds"),
        };

        // parse queue type
        let queueTypeStr = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get test queue type"),
        };
        let mut queueType = MyFastQueue;
        let queueTypeStr = queueTypeStr.to_uppercase();
        if queueTypeStr.eq(&"MYFQ".to_string()) {
            queueType = MyFastQueue;
        } else if queueTypeStr.eq(&"MYMPMC1".to_string()) {
            queueType = MyMPMC1;
        } else if queueTypeStr.eq(&"MYMPMC2".to_string()) {
            queueType = MyMPMC2;
        } else if queueTypeStr.eq(&"MPSC".to_string()) {
            queueType = StdMpsc;
        } else if queueTypeStr.eq(&"MPSCQ".to_string()) {
                queueType = StdMpscQ;
        } else {
            return Err("invalid queue type.");
        }

        return Ok(QueueConf{
            queueType,
            dataSize,
            nSender,
            nReceiver,
            duration
        });
    }
}

pub trait NewData {
    fn new() -> Self;
}

struct S8(i64);
impl NewData for S8 {
    fn new() -> Self {
        S8(0)
    }
}
struct S32(S8, S8, S8, S8);
impl NewData for S32 {
    fn new() -> Self { S32 { 0: S8(1), 1: S8(2), 2: S8(3), 3: S8(4) } }
}
struct S64(S32, S32);
impl NewData for S64 {
    fn new() -> Self { S64 { 0: S32::new(), 1: S32::new() } }
}
struct S128(S64, S64);
impl NewData for S128 {
    fn new() -> Self { S128 { 0: S64::new(), 1: S64::new() } }
}
struct S256(S128, S128);
impl NewData for S256 {
    fn new() -> S256 { S256 { 0: S128::new(), 1: S128::new() } }
}
struct S512(S256, S256);
impl NewData for S512 {
    fn new() -> S512 { S512 { 0: S256::new(), 1: S256::new() } }
}
struct S1024(S512, S512);
impl NewData for S1024 {
    fn new() -> S1024 { S1024 { 0: S512::new(), 1: S512::new() } }
}
struct S2048(S1024, S1024);
impl NewData for S2048 {
    fn new() -> S2048 { S2048 { 0: S1024::new(), 1: S1024::new() } }
}

pub const TEST_QUEUE_SIZE : usize = 100000000;

fn run_q_test<T:'static+NewData+Send+Sync>(config: QueueConf)
    -> Result<(), &'static str>{
    if config.queueType == MyFastQueue {
        return testfq::run_queue_test::<T>(config.nSender, config.nReceiver, config.duration);
    } else if config.queueType == MyMPMC1 {
        return testmpmcq::run_queue_test::<T>(config.nSender, config.nReceiver, config.duration);
    } else if config.queueType == MyMPMC2 {
        return testmpmcq2::run_queue_test::<T>(config.nSender, config.nReceiver, config.duration);
    } else if config.queueType == StdMpsc {
        return testmpsc::run_mpsc_test::<T>(config.nSender, config.nReceiver, config.duration);
    } else if config.queueType == StdMpscQ {
        return testmpscq::run_mpscq_test::<T>(config.nSender, config.nReceiver, config.duration);
    }

    Ok(())
}


pub fn run_queue_test(config: QueueConf) -> Result<(), &'static str> {
    let r =
    if config.dataSize <= 8 {
        run_q_test::<S8>(config)
    } else if config.dataSize <= 128 {
        run_q_test::<S128>(config)
    } else if config.dataSize <= 256 {
        run_q_test::<S256>(config)
    } else if config.dataSize <= 512 {
        run_q_test::<S512>(config)
    } else if config.dataSize <= 1024 {
        run_q_test::<S1024>(config)
    } else if config.dataSize <= 2048 {
        run_q_test::<S2048>(config)
    } else {
        Err("data size is too big")
    };

    return r;
}
