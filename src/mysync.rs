//#![warn(unused_imports)]

use std::sync::{Mutex, Condvar, MutexGuard};
use std::marker::Send;
use std::cell::{UnsafeCell, RefCell};
use std::mem::swap;

///
/// my condition
///
/// # example
///
/// ```
/// use std::sync::Arc;
/// use rust1::mysync::MyCond;
///
/// let cond = false;
/// let c = Arc::new(MyCond::new());
/// let mut guard = c.lock();
/// while (!cond) {
///     guard = c.wait(guard);
/// }
/// c.unlock(guard);
/// ```
///
pub struct MyCond{
    lock : Mutex<()>,
    cond : Condvar,
}

impl MyCond{
    ///
    /// create a MyCondition
    ///
    pub fn new() -> MyCond {
        MyCond{ lock: Mutex::new(()), cond: Default::default()}
    }

    pub fn lock(&self) -> MutexGuard<()> {
        self.lock.lock().unwrap()
    }

    pub fn unlock(&self, lock : MutexGuard<()>) {

    }

    pub fn wait<'a>(&self, lock: MutexGuard<'a, ()>) -> MutexGuard<'a, ()> {
        self.cond.wait(lock).unwrap()
    }

    pub fn signal(&self) {
        self.cond.notify_one();
    }

    pub fn broadcast(&self) {
        self.cond.notify_all();
    }
}

///
/// resource counter
pub struct RC1 {
    count : UnsafeCell<isize>,
    cond : MyCond,
}
impl RC1 {
    fn new() -> RC1 {
        RC1{ count: UnsafeCell::new(0), cond: MyCond::new() }
    }
    fn new_count(count : isize) -> RC1 {
        RC1{ count: UnsafeCell::new(count), cond: MyCond::new() }
    }

    fn Aquire(&self) {
        let mut g = self.cond.lock();

        let counter = unsafe {
            let x = self.count.get();
            *x -= 1;
            *x
        };

        if counter < 0 {
            g = self.cond.wait(g);
        }
        self.cond.unlock(g);
    }

    fn Release(&self) {
        let g = self.cond.lock();
        let counter = unsafe {
            let x = self.count.get();
            *x += 1;
            *x
        };
        if counter <= 0 {
            self.cond.signal();
        }
        self.cond.unlock(g);
    }
}
unsafe impl Send for RC1 {}
unsafe impl Sync for RC1 {}

pub struct RC2 {
    counter : Mutex<isize>,
    cond : Condvar,
}
impl RC2 {
    fn new() -> RC2 {
        RC2{ counter: Mutex::new(0), cond: Default::default() }
    }
    fn new_count(c : isize) -> RC2 {
        RC2{ counter: Mutex::new(c), cond: Default::default() }
    }

    fn Aquire(&self) {
        let mut c = self.counter.lock().unwrap();
        *c -= 1;
        if *c < 0 {
            c = self.cond.wait(c).unwrap();
        }
        println!("aquire: {}", *c);
    }

    fn Release(&self) {
        let mut c = self.counter.lock().unwrap();
        *c += 1;
        println!("release: {}", *c);
        if *c <= 0 {
            self.cond.notify_one();
        }
    }
}

pub struct RC3 {
    count : isize,
    cond : MyCond,
}
impl RC3 {
    fn new() -> RC3{
        RC3{ count: 0, cond: MyCond::new() }
    }
    fn new_count(count : isize) -> RC3 {
        RC3{ count, cond: MyCond::new() }
    }

    fn Aquire(&self) {
        let mut g = self.cond.lock();

        let counter = unsafe {
            let x = &self.count as *const isize as *mut isize;
            *x -= 1;
            *x
        };

        if counter < 0 {
            g = self.cond.wait(g);
        }
        self.cond.unlock(g);
    }

    fn Release(&self) {
        let g = self.cond.lock();
        let counter = unsafe {
            let x = &self.count as *const isize as *mut isize;
            *x += 1;
            *x
        };
        if counter <= 0 {
            self.cond.signal();
        }
        self.cond.unlock(g);
    }
}

pub struct RC4 {
    count : RefCell<isize>,
    cond : MyCond,
}
unsafe impl Send for RC4{}
unsafe impl Sync for RC4{}
impl RC4 {
    fn new() -> RC4{
        RC4{ count: RefCell::new(0), cond: MyCond::new() }
    }
    fn new_count(count : isize) -> RC4 {
        RC4{ count: RefCell::new(count), cond: MyCond::new() }
    }

    fn Aquire(&self) {
        let mut g = self.cond.lock();

        let mut c = self.count.borrow_mut();
        *c -= 1;

        if *c < 0 {
            std::mem::drop(c);
            g = self.cond.wait(g);
        }
        self.cond.unlock(g);
    }

    fn Release(&self) {
        let g = self.cond.lock();
        let mut c = self.count.borrow_mut();
        *c += 1;
        if *c <= 0 {
            self.cond.signal();
        }
        self.cond.unlock(g);
    }
}

pub struct MyAutoEvent<T> {
    signal : Option<T>,
    l : Mutex<usize>,
    c : Condvar,
}
impl<T> MyAutoEvent<T> {
    /// new a event without signal
    pub fn new() -> MyAutoEvent<T> {
        MyAutoEvent {
            signal: None,
            l: Mutex::new(0),
            c: Default::default()
        }
    }

    /// new a event with a signal value
    pub fn from_signal_value(v : T) -> MyAutoEvent<T> {
        MyAutoEvent {
            signal: Some(v),
            l: Mutex::new(0),
            c: Default::default()
        }
    }

    pub fn set_event_value(&mut self, value : T) {
        let g = self.l.lock().unwrap();
        self.signal = Some(value);
        if *g > 0 {
            self.c.notify_all();
        }
        drop(g);
    }

    pub fn wait(&mut self) -> T {
        let mut g = self.l.lock().unwrap();
        *g += 1;
        let rv = loop{
            let mut sig : Option<T> = None;
            swap(&mut sig, &mut self.signal);
            if let Some(v) = sig {
                break v;
            } else {
                g = self.c.wait(g).unwrap();
            }
        };
        *g -= 1;
        return rv;
    }
}
unsafe impl<T> Send for MyAutoEvent<T>{}
unsafe impl<T> Sync for MyAutoEvent<T>{}


#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use crate::mysync::{MyCond, RC2, RC1, RC3, RC4};
    use std::thread::Thread;
    use std::{thread, time};
    use std::sync::Arc;
    use std::borrow::BorrowMut;
    use std::cell::RefMut;
    use std::mem::swap;
    //use crate::dumpDref;
    use crate::dump::dumpRef;


    #[test]
    fn test1() {
        let c1 = Arc::new(MyCond::new());
        let c2 = c1.clone();
        thread::spawn(move || {
            //let c3 = c2;
            println!("child thread begin");
            let mut g = c2.lock();
            g = c2.wait(g);
            c2.unlock(g);
            println!("child thread sleep 5s...");
            thread::sleep(time::Duration::from_micros(5000000));
            c2.signal(); // MyCond::signal(&c2)
            println!("child thread exit.");
        });

        thread::sleep(time::Duration::from_millis(2000));
        println!("main thread notify child to run");
        c1.signal();
        thread::sleep(time::Duration::from_millis(2000));
        println!("main thread wait for signal...{:?}", time::SystemTime::now());

        let mut lock = c1.lock();
        lock = c1.wait(lock);
        c1.unlock(lock);

        println!("main thread received signal   {:?}", time::SystemTime::now());
    }


    #[test]
    fn test_resc(){


        let rc = Arc::new(RC4::new());
        //let x=Arc::get_mut(&mut rc).expect("unwrap rc before start child");
        for i in 1..10 {
            let rc2 = Arc::clone(&rc);
            thread::spawn(move ||{
                println!("child {} started.", i);
                //let myRc = Arc::get_mut(&mut rc2).expect("unwrap rc2 in child");
                thread::sleep(time::Duration::from_millis(i*1000));
                rc2.Release();
            });
        }

        println!("main thread ...");
        thread::sleep(time::Duration::from_millis(1000));
        //let rc=Arc::get_mut(&mut rc).expect("unwrap rc after start child");
        for i in 1..10{
            rc.Aquire();
            println!("go a resource. {:?}", time::Instant::now());
        }
    }

    #[test]
    fn test_vec() {
        #[derive(Debug)]
        struct S{
            pub x : i32,
        }
        impl Drop for S{
            fn drop(&mut self) {
                println!("drop {:?}", self);
            }
        }

        let mut a = [S{x:1}, S{x:2}];
        //let a0 = a[0];
        a[1] = S{x:3};
        println!("{:?}", a);
        let mut b = S{x:4};
        swap(&mut a[0], &mut b);
        println!("{:?}", a);

        let mut v = Vec::<S>::with_capacity(10);
        println!("{:?}", v);
        dumpRef("v", &v);
        for i in 0..10 {
            v.push(S{x: i as i32});
        }

        dumpRef("v5", &v[5]);
        v[5] = S{x:50};
        dumpRef("v5", &v[5]);
    }
}