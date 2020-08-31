use rayon::prelude::*;
use std::collections::BTreeMap;

pub fn power_iteration<S, T>(
    edges: S,
    stride: usize,
    iterations: usize,
) -> impl Iterator<Item = (T, f32)>
where
    T: Ord + Clone,
    S: Iterator<Item = (T, T)>,
{
    log::info!("starting PageRank power iteration...");

    let mut index = BTreeMap::new();
    let mut states = Vec::new();
    let mut n_states = 0; // for convenience...
    let mut transition = Vec::new();

    let mut id_for = |node: T| {
        *index.entry(node.clone()).or_insert_with(|| {
            let new = n_states;
            states.push(node);
            n_states += 1;
            new
        }) as u32
    };

    // Materialize everything:
    log::info!("fetching data");
    for (from, to) in edges {
        let from_id = id_for(from);
        let to_id = id_for(to);

        transition.push((from_id, to_id));
    }

    log::info!("loaded {} canonical pages in total", n_states);

    // Sort (better than cache missing like crazy?):
    transition.sort_unstable_by_key(|&(from_id, _)| from_id);

    log::info!("sorted");

    let mut offset_length = vec![(std::usize::MAX, 0); n_states];
    for (i, &(from_id, _)) in transition.iter().enumerate() {
        let (offset_min, offset_max) = offset_length[from_id as usize];
        offset_length[from_id as usize] = (usize::min(offset_min, i), usize::max(offset_max, i));
    }

    // Now, for the magic: sparse, tiled left-multiplication.
    let n_strides = if n_states % stride == 0 {
        n_states / stride
    } else {
        n_states / stride + 1
    };
    let mut state = vec![1. / n_states as f32; n_states];

    for iter in 0..iterations {
        log::info!("iteration {}", iter + 1);

        let mut new_state = vec![1. / n_states as f32; n_states];

        let pieces = (0..n_strides)
            .map(|i| (0..n_strides).map(move |j| (i, j)))
            .flatten()
            .par_bridge()
            .map(|(i, j)| {
                let mut batch = vec![0.; stride];
                let (min_j, sup_j) = (j * stride, usize::min(n_states, (j + 1) * stride));
                let (min_i, sup_i) = (i * stride, usize::min(n_states, (i + 1) * stride));

                for (i, &(offset_min, offset_max)) in offset_length[min_i..sup_i].iter().enumerate()
                {
                    let from_id = min_i + i;
                    if offset_min != std::usize::MAX {
                        let individual_share =
                            1. / (offset_max - offset_min + 1) as f32 * state[from_id];

                        for &(e_from, to_id) in &transition[offset_min..=offset_max] {
                            assert_eq!(e_from, from_id as u32);
                            let to_id = to_id as usize; // inflate
                            if to_id >= min_j && to_id < sup_j {
                                batch[to_id - min_j] += individual_share;
                            }
                        }
                    }
                }

                (min_j, batch)
            })
            .fold(
                BTreeMap::<_, Vec<f32>>::new,
                |mut acc, (min_j, batch)| {
                    acc.entry(min_j)
                        .and_modify(|existing| {
                            for (existing, new) in existing.iter_mut().zip(&batch) {
                                *existing += *new;
                            }
                        })
                        .or_insert(batch);

                    acc
                },
            )
            .reduce(
                BTreeMap::<_, Vec<f32>>::new,
                |mut a, b| {
                    for (min_j, batch) in b {
                        a.entry(min_j)
                            .and_modify(|existing| {
                                for (partial, new) in existing.iter_mut().zip(&batch) {
                                    *partial += *new;
                                }
                            })
                            .or_insert(batch);
                    }

                    a
                },
            );

        // Find out lost juice:
        let lost_juice = state
            .par_iter()
            .zip(&offset_length)
            .filter(|(_, &(offset_min, _))| offset_min == std::usize::MAX)
            .map(|(&state, _)| state)
            .sum::<f32>();

        // Assemble + random walk factor:
        let restart_diffusion = (0.15 + 0.85 * lost_juice) / n_states as f32;
        for (min_j, piece) in pieces {
            for (j, &piece_j) in piece.iter().enumerate() {
                // Batch might be sightly too big in some cases....
                if let Some(new_state_j) = new_state.get_mut(min_j + j) {
                    *new_state_j = piece_j * 0.85 + restart_diffusion;
                }
            }
        }

        // Log normalization:
        let norm = new_state.iter().cloned().sum::<f32>();
        log::info!("norm: {}", norm);

        // Log KL divergence:
        let kl_div = new_state
            .iter()
            .zip(&*state)
            .map(|(&qi, &pi)| -pi * (qi / pi).log2())
            .sum::<f32>();
        log::info!("kl divergence: {}", kl_div);

        // Swap states:
        state = new_state;
    }

    log::info!("done");

    states.into_iter().zip(state)
}
