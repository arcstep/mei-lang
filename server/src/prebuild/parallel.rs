use super::prelude::*;
use super::*;

pub(crate) fn prebuild_parallelism(job_count: usize) -> usize {
    if job_count <= 1 {
        return 1;
    }
    thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1)
        .min(prebuild_max_parallelism_cap())
        .min(job_count)
        .max(1)
}

pub(crate) fn run_limited_parallel_ordered<T, R, F>(
    items: Vec<T>,
    max_parallelism: usize,
    job: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Sync,
{
    run_limited_parallel_ordered_with_hook(items, max_parallelism, job, |_, _| {})
}

pub(crate) fn run_limited_parallel_ordered_with_hook<T, R, F, H>(
    items: Vec<T>,
    max_parallelism: usize,
    job: F,
    hook: H,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Sync,
    H: Fn(usize, &R) + Sync,
{
    if items.len() <= 1 || max_parallelism <= 1 {
        return items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let result = job(item);
                hook(index, &result);
                result
            })
            .collect();
    }
    let prebuild_store_override = mei_lang_kernel::snapshot_prebuild_build_root_override();
    let worker_count = max_parallelism.min(items.len()).max(1);
    let mut buckets = (0..worker_count)
        .map(|_| Vec::<(usize, T)>::new())
        .collect::<Vec<_>>();
    for (index, item) in items.into_iter().enumerate() {
        buckets[index % worker_count].push((index, item));
    }
    thread::scope(|scope| {
        let job_ref = &job;
        let hook_ref = &hook;
        let mut handles = Vec::new();
        for bucket in buckets.into_iter().filter(|bucket| !bucket.is_empty()) {
            let prebuild_store_override = prebuild_store_override.clone();
            handles.push(scope.spawn(move || {
                mei_lang_kernel::restore_prebuild_build_root_override(
                    prebuild_store_override.clone(),
                );
                let mut output = Vec::with_capacity(bucket.len());
                for (index, item) in bucket {
                    let result = job_ref(item);
                    hook_ref(index, &result);
                    output.push((index, result));
                }
                mei_lang_kernel::restore_prebuild_build_root_override(None);
                output
            }));
        }
        let mut output = Vec::new();
        for handle in handles {
            output.extend(handle.join().expect("prebuild parallel worker panicked"));
        }
        output.sort_by_key(|(index, _)| *index);
        output.into_iter().map(|(_, result)| result).collect()
    })
}
