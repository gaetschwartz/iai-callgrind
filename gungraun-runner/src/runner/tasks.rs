//! TODO: DOCS

use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Output};
use std::sync::atomic::{self, AtomicBool};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;
use std::{iter, thread};

use anyhow::{anyhow, Context, Result};
use crossbeam::deque::{Injector, Steal, Stealer, Worker};
use log::debug;
use nix::sys::signal;
use nix::unistd::Pid;

use super::common::AssistantKind;
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::common::{Assistant, Config, ModulePath, Streams};
use crate::runner::tool::config::ToolConfig;
use crate::runner::tool::path::ToolOutputPath;
use crate::runner::tool::run::{check_exit, RunOptions, ToolCommand, ToolCommandChild};

type Channel<T> = (Sender<(JobId, T)>, Receiver<(JobId, T)>);
type Job<T> = (JobId, JobClosure<T>);
type JobClosure<T> = Box<dyn FnOnce(Arc<AtomicBool>) -> T + Send + 'static>;
type JobId = usize;
type TaskHandle = JoinHandle<Result<()>>;

/// TODO: DOCS
struct ProcessChild(Child);

/// TODO: DOCS
#[derive(Debug)]
pub struct ProcessHandler {
    /// TODO: DOCS
    pub bench: Option<ToolCommandChild>,
    /// TODO: DOCS
    pub force_shutdown: Arc<AtomicBool>,
    /// TODO: DOCS
    pub module_path: ModulePath,
    /// TODO: DOCS
    pub poll_interval: Duration,
    /// TODO: DOCS
    pub sandbox_dir: Option<PathBuf>,
    /// TODO: DOCS
    pub setup: Option<(String, Child)>,
    /// TODO: DOCS
    pub setup_is_parallel: bool,
    /// TODO: DOCS
    pub teardown: Option<(String, Child)>,
}

#[derive(Debug, Clone, Copy)]
enum ProcessState {
    Running,
    Term,
    Kill,
}

// Worker struct
struct Task {
    thread: Option<TaskHandle>,
}

/// TODO: DOCS
/// A thread pool that returns jobs in their insertion order. Efficient work-stealing
/// implementation.
pub struct ThreadPool<T> {
    force_shutdown: Arc<AtomicBool>,
    graceful_shutdown: Arc<AtomicBool>,
    job_queue: Arc<Injector<Job<T>>>,
    next: Option<JobId>,
    num_jobs: Option<usize>,
    num_received: usize,
    result_receiver: Receiver<(JobId, T)>,
    results: HashMap<usize, T>,
    tasks: Vec<Task>,
}

impl ProcessChild {
    fn wait(self, force_shutdown: &Arc<AtomicBool>, poll_interval: Duration) -> Result<Output> {
        let mut run_state = ProcessState::Running;
        // This should be enough time for a proper shutdown of any benchmark process
        let mut ticks = 100;
        let mut child = self.0;
        let mut interrupted = false;

        loop {
            match child.try_wait() {
                Ok(Some(_)) if interrupted => {
                    break Err(Error::TaskInterrupt.into());
                }
                Ok(Some(_)) => {
                    break Ok(child
                        .wait_with_output()
                        .expect("The output should be present if there is an exit status"));
                }
                Ok(None) => {
                    match run_state {
                        ProcessState::Running if force_shutdown.load(atomic::Ordering::Acquire) => {
                            let pid_t = i32::try_from(child.id())?;
                            let pid = Pid::from_raw(pid_t);
                            signal::kill(pid, signal::SIGTERM)?;

                            run_state = ProcessState::Term;
                            interrupted = true;
                        }
                        ProcessState::Running | ProcessState::Kill => {}
                        ProcessState::Term if ticks > 0 => {
                            ticks -= 1;
                        }
                        ProcessState::Term => {
                            child.kill()?;
                            run_state = ProcessState::Kill;
                        }
                    }
                    sleep(poll_interval);
                }
                Err(error) => {
                    break Err(error)
                        .with_context(|| "Trying to wait for the benchmark process to stop");
                }
            }
        }
    }
}

impl ProcessHandler {
    /// TODO: DOCS
    pub fn new(
        force_shutdown: Arc<AtomicBool>,
        module_path: ModulePath,
        setup_is_parallel: bool,
        poll_interval: Duration,
        sandbox_dir: Option<&Path>,
    ) -> Self {
        Self {
            bench: None,
            force_shutdown,
            module_path,
            setup: None,
            setup_is_parallel,
            teardown: None,
            poll_interval,
            sandbox_dir: sandbox_dir.map(Path::to_path_buf),
        }
    }

    /// TODO: DOCS
    pub fn start_assistant(
        &mut self,
        force_parallel: bool,
        assistant: &Assistant,
        config: &Config,
        module_path: &ModulePath,
        streams: Option<&Streams>,
        nocapture: NoCapture,
    ) -> Result<()> {
        if self.force_shutdown.load(atomic::Ordering::Acquire) {
            return Err(Error::TaskInterrupt.into());
        }

        match assistant.kind() {
            AssistantKind::Setup => {
                let child = assistant.run(
                    config,
                    module_path,
                    streams,
                    force_parallel,
                    self.sandbox_dir.as_deref(),
                    nocapture,
                )?;
                self.setup_is_parallel = assistant.is_parallel();
                self.setup = child.map(|c| (assistant.kind().id(), c));
            }
            AssistantKind::Teardown => {
                let child = assistant.run(
                    config,
                    module_path,
                    streams,
                    force_parallel,
                    self.sandbox_dir.as_deref(),
                    nocapture,
                )?;
                self.teardown = child.map(|c| (assistant.kind().id(), c));
            }
        }

        Ok(())
    }

    /// TODO: DOCS
    pub fn start_bench(
        &mut self,
        command: ToolCommand,
        tool_config: ToolConfig,
        executable: &Path,
        executable_args: &[OsString],
        run_options: RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        streams: Option<&Streams>,
    ) -> Result<()> {
        if !self.setup_is_parallel {
            if let Some(Err(error)) = self.wait_for_setup() {
                return Err(error);
            }
        }

        if self.force_shutdown.load(atomic::Ordering::Acquire) {
            return Err(Error::TaskInterrupt.into());
        }

        let child = command.run(
            tool_config,
            executable,
            executable_args,
            run_options,
            output_path,
            module_path,
            self.setup.as_mut().map(|(_, c)| c),
            streams,
            self.sandbox_dir.as_deref(),
        )?;

        self.bench = Some(child);

        Ok(())
    }

    /// TODO: DOCS
    pub fn wait_or_shutdown(&mut self) -> Result<Option<Output>> {
        let mut bench_child = self
            .bench
            .take()
            .expect("A benchmark should be started before waiting");

        let result = ProcessChild(
            bench_child
                .child
                .take()
                .expect("A child process should be present"),
        )
        .wait(&self.force_shutdown, self.poll_interval)
        .with_context(|| "Trying to wait for the benchmark process to stop")
        .and_then(|output| {
            let status = output.status;
            check_exit(
                bench_child.tool,
                &bench_child.executable,
                Some(output),
                status,
                &bench_child.log_path,
                bench_child.exit_with.as_ref(),
            )
        });

        if let Some(Err(error)) = self.wait_for_setup() {
            return Err(error);
        }

        result
    }

    fn wait_for_assistant(&self, child: Child, id: &str) -> Result<()> {
        ProcessChild(child)
            .wait(&self.force_shutdown, self.poll_interval)
            .with_context(|| format!("Trying to wait for the {id} process to stop"))
            .and_then(|output| {
                let status = output.status;
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::new_process_error(
                        self.module_path.join(id).to_string(),
                        Some(output),
                        status,
                        None,
                    )
                    .into())
                }
            })
    }

    /// TODO: DOCS
    pub fn wait_for_setup(&mut self) -> Option<Result<()>> {
        if let Some((id, child)) = self.setup.take() {
            debug!("Waiting for setup to complete");
            return Some(self.wait_for_assistant(child, &id));
        }

        None
    }

    /// TODO: DOCS
    pub fn wait_for_teardown(&mut self) -> Option<Result<()>> {
        if let Some((id, child)) = self.teardown.take() {
            debug!("Waiting for teardown to complete");
            return Some(self.wait_for_assistant(child, &id));
        }

        None
    }
}

impl Task {
    fn new(thread: TaskHandle) -> Self {
        Self {
            thread: Some(thread),
        }
    }
}

impl<T: Send + 'static> ThreadPool<T> {
    /// TODO: DOCS
    pub fn new(size: usize) -> Result<Self> {
        if size < 1 {
            return Err(anyhow!(
                "Minimum size for a thread pool is 1 but was: '{size}'"
            ));
        }

        let (result_sender, result_receiver): Channel<T> = mpsc::channel();

        let force_shutdown = Arc::new(AtomicBool::new(false));
        let graceful_shutdown = Arc::new(AtomicBool::new(false));
        let injector = Arc::new(Injector::<Job<T>>::new());

        let mut local_queues = VecDeque::<Worker<Job<T>>>::with_capacity(size);
        let mut stealers = Vec::<Stealer<Job<T>>>::with_capacity(size);

        for _ in 0..size {
            let queue = Worker::<Job<T>>::new_fifo();
            stealers.push(queue.stealer());
            local_queues.push_back(queue);
        }

        let mut tasks = Vec::with_capacity(size);
        for _ in 0..size {
            let result_sender = result_sender.clone();
            let global_queue = Arc::clone(&injector);

            // This unwrap is safe since this loop and the one to fill the local queue iterate over
            // the same amount of elements
            let local_queue = local_queues.pop_front().unwrap();
            let local_stealers = stealers.clone();
            let graceful_shutdown = Arc::clone(&graceful_shutdown);
            let force_shutdown = Arc::clone(&force_shutdown);

            let thread: TaskHandle = thread::spawn(move || {
                loop {
                    if force_shutdown.load(atomic::Ordering::Acquire) {
                        break;
                    }

                    let job = local_queue.pop().or_else(|| {
                        iter::repeat_with(|| {
                            global_queue
                                .steal_batch_and_pop(&local_queue)
                                .or_else(|| local_stealers.iter().map(Stealer::steal).collect())
                        })
                        .find(|s| !s.is_retry())
                        .and_then(Steal::success)
                    });

                    if let Some((id, job)) = job {
                        let force_shutdown = Arc::clone(&force_shutdown);

                        let result = job(force_shutdown);
                        result_sender
                            .send((id, result))
                            .map_err(|error| anyhow!("{error}"))?;
                    } else if graceful_shutdown.load(atomic::Ordering::Acquire) {
                        break;
                    } else {
                        std::hint::spin_loop();
                    }
                }

                Ok(())
            });

            tasks.push(Task::new(thread));
        }

        Ok(Self {
            tasks,
            result_receiver,
            graceful_shutdown,
            force_shutdown,
            job_queue: injector,
            results: HashMap::new(),
            next: None,
            num_jobs: None,
            num_received: 0,
        })
    }

    /// TODO: DOCS
    pub fn get_force_shutdown(&self) -> Arc<AtomicBool> {
        self.force_shutdown.clone()
    }

    /// TODO: DOCS
    pub fn execute<F>(&mut self, job: F)
    where
        F: FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
    {
        let num_jobs = self.num_jobs.get_or_insert(0);
        self.job_queue.push((*num_jobs, Box::new(job)));
        *num_jobs += 1;
    }

    /// TODO: DOCS
    pub fn shutdown(&mut self) {
        self.graceful_shutdown
            .store(true, atomic::Ordering::Release);
        for task in &mut self.tasks {
            if let Some(thread) = task.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

impl<T> Iterator for ThreadPool<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.next == self.num_jobs {
                break None;
            } else if self.num_jobs.is_some_and(|c| c == self.num_received) {
                if let Some(next) = self.next.as_mut() {
                    let result = self.results.remove(next);
                    *next += 1;
                    break result;
                }

                break None;
            } else if let Ok((index, result)) = self.result_receiver.recv() {
                let next = self.next.get_or_insert(0);
                self.num_received += 1;

                #[allow(clippy::else_if_without_else)]
                if index == *next {
                    *next += 1;
                    break Some(result);
                } else if let Some(r) = self.results.remove(next) {
                    self.results.insert(index, result);
                    *next += 1;
                    break Some(r);
                }

                self.results.insert(index, result);
            } else {
                break None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rstest::rstest;

    use super::*;
    use crate::api::LibraryBenchmarkConfig;
    use crate::runner::common::ModulePath;
    use crate::runner::lib_bench::{self, LibBench};
    use crate::runner::meta::Metadata;

    #[rstest]
    #[case::size_one_jobs_zero(1, 0)]
    #[case::equal_one(1, 1)]
    #[case::size_one_jobs_two(1, 2)]
    #[case::size_one_jobs_three(1, 3)]
    #[case::size_one_jobs_twenty(1, 20)]
    #[case::size_two_jobs_1(2, 1)]
    #[case::equal_two(2, 2)]
    #[case::size_two_jobs_3(2, 3)]
    #[case::size_two_jobs_4(2, 4)]
    #[case::size_two_jobs_20(2, 20)]
    #[case::size_19_jobs_20(19, 20)]
    #[case::equal_twenty(20, 20)]
    #[case::size_21_jobs_20(21, 20)]
    fn test_thread_pool_execute_and_next(#[case] size: usize, #[case] jobs: usize) {
        let mut pool = ThreadPool::new(size).unwrap();
        for i in 0..jobs {
            pool.execute(move |_| {
                // Simulating some work
                if i % 2 == 0 {
                    Ok(i) // Successful job
                } else {
                    Err(format!("Failed job {i}")) // Simulated failure
                }
            });
        }

        let mut expected = 0;
        for result in pool {
            match result {
                Ok(num) => {
                    assert_eq!(num, expected);
                }
                Err(error) => assert_eq!(error, format!("Failed job {expected}")),
            }

            expected += 1;
        }

        assert_eq!(expected, jobs);
    }

    #[test]
    fn test_thread_pool_next_when_no_execute() {
        let mut pool = ThreadPool::<usize>::new(4).unwrap();
        assert_eq!(pool.tasks.len(), 4);
        assert_eq!(pool.next(), None);
    }

    #[test]
    fn test_thread_pool_when_size_is_zero() {
        assert!(ThreadPool::<usize>::new(0).is_err());
    }

    #[test]
    fn test_thread_pool_with_lib_bench() {
        let meta = Metadata::new(
            &[],
            "benchmark-tests",
            &PathBuf::from("test_lib_bench_intro.rs"),
        )
        .unwrap();
        let bench = lib_bench::LibBench::new(
            None,
            None,
            ModulePath::new("hello::world"),
            "function".to_owned(),
            &meta,
            LibraryBenchmarkConfig::default(),
            0,
            0,
            None,
            crate::api::ValgrindTool::Callgrind,
        )
        .unwrap()
        .unwrap();

        let mut thread_pool = ThreadPool::<LibBench>::new(4).unwrap();
        thread_pool.execute(move |_| bench);

        let next = thread_pool.next();
        assert!(next.is_some());
    }
}
