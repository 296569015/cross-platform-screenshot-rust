use std::cmp::{max, min};

const BYTES_PER_PIXEL: usize = 4;
const FEATURE_BINS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongScreenshotStitchResult {
    pub appended: bool,
    pub duplicate: bool,
    pub reliable: bool,
    pub overlap_rows: i32,
    pub appended_rows: i32,
    pub score: f32,
    pub second_best_score: f32,
}

impl Default for LongScreenshotStitchResult {
    fn default() -> Self {
        Self {
            appended: false,
            duplicate: false,
            reliable: false,
            overlap_rows: 0,
            appended_rows: 0,
            score: 0.0,
            second_best_score: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongScreenshotStitchOptions {
    pub min_overlap_rows: i32,
    pub max_overlap_rows: i32,
    pub min_append_rows: i32,
    pub duplicate_score: f32,
    pub reliable_match_score: f32,
    pub acceptable_match_score: f32,
    pub ambiguous_score_gap: f32,
    pub acceptable_score_gap: f32,
    pub append_on_unreliable_match: bool,
    pub prefer_larger_overlap_score_slack: f32,
}

impl Default for LongScreenshotStitchOptions {
    fn default() -> Self {
        Self {
            min_overlap_rows: 24,
            max_overlap_rows: 900,
            min_append_rows: 6,
            duplicate_score: 1.0,
            reliable_match_score: 16.0,
            acceptable_match_score: 30.0,
            ambiguous_score_gap: 1.5,
            acceptable_score_gap: 0.0,
            append_on_unreliable_match: true,
            prefer_larger_overlap_score_slack: 1.25,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LongScreenshotStitcher {
    width: i32,
    height: i32,
    last_frame_height: i32,
    options: LongScreenshotStitchOptions,
    pixels: Vec<u8>,
    last_frame: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
struct RowFeature {
    luma: f32,
    contrast: f32,
    edge: f32,
    color: f32,
    bins: [f32; FEATURE_BINS],
}

#[derive(Clone, Copy, Debug)]
struct OverlapCandidate {
    overlap: i32,
    row_score: f32,
    pixel_score: f32,
}

impl LongScreenshotStitcher {
    pub fn new(width: i32, options: LongScreenshotStitchOptions) -> Self {
        Self {
            width,
            height: 0,
            last_frame_height: 0,
            options,
            pixels: Vec::new(),
            last_frame: Vec::new(),
        }
    }

    pub fn reset(&mut self, width: i32) {
        self.width = width;
        self.height = 0;
        self.last_frame_height = 0;
        self.pixels.clear();
        self.last_frame.clear();
    }

    pub fn start(&mut self, first_frame: &[u8], frame_height: i32) {
        if self.width <= 0 || frame_height <= 0 {
            self.clear_frames();
            return;
        }

        let expected_size = frame_len(self.width, frame_height);
        if first_frame.len() < expected_size {
            self.clear_frames();
            return;
        }

        self.height = frame_height;
        self.pixels = first_frame[..expected_size].to_vec();
        self.last_frame_height = frame_height;
        self.last_frame = first_frame[..expected_size].to_vec();
    }

    pub fn append(
        &mut self,
        next_frame: &[u8],
        frame_height: i32,
        allow_acceptable_match: bool,
    ) -> LongScreenshotStitchResult {
        let mut result = LongScreenshotStitchResult::default();
        if self.width <= 0
            || self.height <= 0
            || frame_height <= 0
            || self.last_frame.is_empty()
            || self.last_frame_height <= 0
        {
            return result;
        }

        let expected_size = frame_len(self.width, frame_height);
        if next_frame.len() < expected_size {
            return result;
        }
        let next_frame = &next_frame[..expected_size];

        let duplicate_score = self.compare_duplicate(next_frame, frame_height);
        if duplicate_score <= self.options.duplicate_score {
            result.duplicate = true;
            result.score = duplicate_score;
            result.overlap_rows = frame_height;
            return result;
        }

        let max_overlap = min(
            self.options.max_overlap_rows,
            min(self.last_frame_height, frame_height - 1),
        );
        let min_overlap = min(self.options.min_overlap_rows, max_overlap);
        if max_overlap <= 0 || min_overlap <= 0 || min_overlap > max_overlap {
            return result;
        }

        let previous_features =
            build_row_features(&self.last_frame, self.width, self.last_frame_height);
        let next_features = build_row_features(next_frame, self.width, frame_height);

        let mut candidates = Vec::new();
        let collect_overlap = |candidates: &mut Vec<OverlapCandidate>, overlap: i32| {
            let score = compare_feature_overlap(
                &previous_features,
                self.last_frame_height,
                &next_features,
                frame_height,
                overlap,
            );
            add_candidate(candidates, overlap, score);
        };

        let overlap_range = max_overlap - min_overlap + 1;
        let coarse_step = if overlap_range > 96 {
            max(2, overlap_range / 32)
        } else {
            1
        };
        let mut overlap = min_overlap;
        while overlap <= max_overlap {
            collect_overlap(&mut candidates, overlap);
            overlap += coarse_step;
        }

        if coarse_step > 1 && !candidates.is_empty() {
            let mut ranked = candidates.clone();
            ranked.sort_by(|a, b| {
                a.row_score
                    .total_cmp(&b.row_score)
                    .then_with(|| b.overlap.cmp(&a.overlap))
            });

            for seed in ranked.into_iter().take(4) {
                let refine_start = max(min_overlap, seed.overlap - coarse_step);
                let refine_end = min(max_overlap, seed.overlap + coarse_step);
                for refined in refine_start..=refine_end {
                    collect_overlap(&mut candidates, refined);
                }
            }
        }

        for candidate in &mut candidates {
            candidate.pixel_score = compare_pixel_overlap(
                &self.last_frame,
                self.width,
                self.last_frame_height,
                next_frame,
                frame_height,
                candidate.overlap,
            );
        }
        candidates.sort_by(|a, b| {
            a.pixel_score
                .total_cmp(&b.pixel_score)
                .then_with(|| b.overlap.cmp(&a.overlap))
        });

        let original_best_score = candidates.first().map_or(f32::MAX, |c| c.pixel_score);
        let mut selected_index = 0;
        if !candidates.is_empty() {
            let score_ceiling =
                original_best_score + self.options.prefer_larger_overlap_score_slack;
            for (index, candidate) in candidates.iter().enumerate().skip(1) {
                if candidate.pixel_score <= score_ceiling
                    && candidate.overlap > candidates[selected_index].overlap
                {
                    selected_index = index;
                }
            }
        }

        let best_overlap = candidates.get(selected_index).map_or(0, |c| c.overlap);
        let best_score = candidates
            .get(selected_index)
            .map_or(f32::MAX, |c| c.pixel_score);
        let mut second_best_score = f32::MAX;
        for (index, candidate) in candidates.iter().enumerate() {
            if index == selected_index {
                continue;
            }
            if (candidate.overlap - best_overlap).abs() > 2 {
                second_best_score = candidate.pixel_score;
                break;
            }
        }

        let has_clear_winner = !second_best_score.is_finite()
            || second_best_score - best_score >= self.options.ambiguous_score_gap;
        let resolved_by_larger_overlap = selected_index != 0
            && best_score <= original_best_score + self.options.prefer_larger_overlap_score_slack;
        let reliable =
            best_overlap > 0 && best_score <= self.options.reliable_match_score && has_clear_winner;
        let acceptable = best_overlap > 0
            && best_score <= self.options.acceptable_match_score
            && (!second_best_score.is_finite()
                || second_best_score - best_score >= self.options.acceptable_score_gap
                || resolved_by_larger_overlap);

        let append_from = if reliable {
            best_overlap
        } else if allow_acceptable_match && self.options.append_on_unreliable_match && acceptable {
            best_overlap
        } else {
            result.score = best_score;
            result.second_best_score = second_best_score;
            result.overlap_rows = best_overlap;
            return result;
        };

        let append_rows = frame_height - append_from;
        if append_rows < self.options.min_append_rows {
            result.duplicate = true;
            result.score = best_score;
            result.second_best_score = second_best_score;
            result.overlap_rows = best_overlap;
            return result;
        }

        let append_bytes = frame_len(self.width, append_rows);
        let append_offset = row_offset(self.width, append_from);
        self.pixels
            .extend_from_slice(&next_frame[append_offset..append_offset + append_bytes]);
        self.height += append_rows;

        result.appended = true;
        result.reliable = reliable;
        result.overlap_rows = append_from;
        result.appended_rows = append_rows;
        result.score = best_score;
        result.second_best_score = second_best_score;
        self.last_frame_height = frame_height;
        self.last_frame = next_frame.to_vec();
        result
    }

    pub fn empty(&self) -> bool {
        self.pixels.is_empty()
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn clear_frames(&mut self) {
        self.height = 0;
        self.last_frame_height = 0;
        self.pixels.clear();
        self.last_frame.clear();
    }

    fn compare_duplicate(&self, next_frame: &[u8], frame_height: i32) -> f32 {
        if self.last_frame_height != frame_height || self.last_frame.is_empty() {
            return 255.0;
        }

        let x_step = max(1, self.width / 96);
        let y_step = max(1, frame_height / 96);
        let mut diff = 0_u64;
        let mut samples = 0_u64;

        let mut y = 0;
        while y < frame_height {
            let a = row_offset(self.width, y);
            let b = row_offset(self.width, y);
            let mut x = 0;
            while x < self.width {
                let i = (x as usize) * BYTES_PER_PIXEL;
                diff += self.last_frame[a + i].abs_diff(next_frame[b + i]) as u64;
                diff += self.last_frame[a + i + 1].abs_diff(next_frame[b + i + 1]) as u64;
                diff += self.last_frame[a + i + 2].abs_diff(next_frame[b + i + 2]) as u64;
                samples += 3;
                x += x_step;
            }
            y += y_step;
        }

        if samples == 0 {
            255.0
        } else {
            diff as f32 / samples as f32
        }
    }
}

impl Default for LongScreenshotStitcher {
    fn default() -> Self {
        Self::new(0, LongScreenshotStitchOptions::default())
    }
}

fn frame_len(width: i32, height: i32) -> usize {
    width.max(0) as usize * height.max(0) as usize * BYTES_PER_PIXEL
}

fn row_offset(width: i32, y: i32) -> usize {
    y.max(0) as usize * width.max(0) as usize * BYTES_PER_PIXEL
}

fn pixel_luma(p: &[u8]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn build_row_features(frame: &[u8], width: i32, height: i32) -> Vec<RowFeature> {
    let mut features = vec![RowFeature::default(); height.max(0) as usize];
    if width <= 0 || height <= 0 {
        return features;
    }

    let ignored_right = min(width / 8, max(2, width / 24));
    let x0 = min(width - 1, max(0, width / 80));
    let mut x1 = max(x0 + 1, width - ignored_right);
    if x1 > width {
        x1 = width;
    }

    for y in 0..height {
        let row = row_offset(width, y);
        let mut luma_sum = 0.0_f64;
        let mut luma_sq_sum = 0.0_f64;
        let mut edge_sum = 0.0_f64;
        let mut color_sum = 0.0_f64;
        let mut bin_sums = [0.0_f64; FEATURE_BINS];
        let mut bin_counts = [0_i32; FEATURE_BINS];
        let mut samples = 0_i32;
        let mut previous_luma = 0.0_f32;
        let mut has_previous = false;
        let span = max(1, x1 - x0);

        for x in x0..x1 {
            let i = row + x as usize * BYTES_PER_PIXEL;
            let p = &frame[i..i + BYTES_PER_PIXEL];
            let luma = pixel_luma(p);
            let bin = min(
                FEATURE_BINS as i32 - 1,
                ((x - x0) * FEATURE_BINS as i32) / span,
            ) as usize;
            luma_sum += luma as f64;
            luma_sq_sum += (luma * luma) as f64;
            color_sum += (p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2])) as f64;
            bin_sums[bin] += luma as f64;
            bin_counts[bin] += 1;
            if has_previous {
                edge_sum += (luma - previous_luma).abs() as f64;
            }
            previous_luma = luma;
            has_previous = true;
            samples += 1;
        }

        if samples <= 0 {
            continue;
        }

        let mean = luma_sum / samples as f64;
        let variance = (luma_sq_sum / samples as f64 - mean * mean).max(0.0);
        let mut feature = RowFeature {
            luma: mean as f32,
            contrast: variance.sqrt() as f32,
            edge: (edge_sum / samples as f64) as f32,
            color: (color_sum / samples as f64) as f32,
            bins: [0.0; FEATURE_BINS],
        };
        for i in 0..FEATURE_BINS {
            feature.bins[i] = if bin_counts[i] > 0 {
                (bin_sums[i] / bin_counts[i] as f64) as f32
            } else {
                feature.luma
            };
        }
        features[y as usize] = feature;
    }

    features
}

fn row_information(row: RowFeature) -> f32 {
    row.contrast + row.edge * 2.0 + row.color * 0.35
}

fn compare_feature_overlap(
    previous: &[RowFeature],
    previous_height: i32,
    next: &[RowFeature],
    next_height: i32,
    overlap_rows: i32,
) -> f32 {
    let previous_start_y = previous_height - overlap_rows;
    if previous_start_y < 0 || overlap_rows <= 0 || overlap_rows >= next_height {
        return 255.0;
    }

    let y_step = max(1, overlap_rows / 128);
    let mut diff = 0.0_f64;
    let mut weight_sum = 0.0_f64;
    let mut informative_rows = 0_i32;

    let mut y = 0;
    while y < overlap_rows {
        let a = previous[(previous_start_y + y) as usize];
        let b = next[y as usize];
        let info = row_information(a).max(row_information(b));
        let weight = 1.0 + info.min(96.0) / 28.0;
        let mut bin_diff = 0.0;
        for i in 0..FEATURE_BINS {
            bin_diff += (a.bins[i] - b.bins[i]).abs();
        }
        bin_diff /= FEATURE_BINS as f32;
        diff += weight as f64
            * ((a.luma - b.luma).abs() * 0.35
                + bin_diff * 0.85
                + (a.contrast - b.contrast).abs() * 0.65
                + (a.edge - b.edge).abs() * 1.35
                + (a.color - b.color).abs() * 0.35) as f64;
        weight_sum += weight as f64;
        if info >= 3.0 {
            informative_rows += 1;
        }
        y += y_step;
    }

    if informative_rows < 6 || weight_sum <= 0.0 {
        255.0
    } else {
        (diff / weight_sum) as f32
    }
}

fn compare_pixel_overlap(
    previous: &[u8],
    width: i32,
    previous_height: i32,
    next: &[u8],
    next_height: i32,
    overlap_rows: i32,
) -> f32 {
    let previous_start_y = previous_height - overlap_rows;
    if width <= 0 || previous_start_y < 0 || overlap_rows <= 0 || overlap_rows >= next_height {
        return 255.0;
    }

    let ignored_right = min(width / 8, max(2, width / 24));
    let x_end = max(1, width - ignored_right);
    let x_step = max(1, width / 128);
    let y_step = max(1, overlap_rows / 128);

    let mut diff = 0.0_f64;
    let mut samples = 0_u64;
    let mut y = 0;
    while y < overlap_rows {
        let a = row_offset(width, previous_start_y + y);
        let b = row_offset(width, y);
        let mut x = 0;
        while x < x_end {
            let i = x as usize * BYTES_PER_PIXEL;
            diff += previous[a + i].abs_diff(next[b + i]) as f64;
            diff += previous[a + i + 1].abs_diff(next[b + i + 1]) as f64;
            diff += previous[a + i + 2].abs_diff(next[b + i + 2]) as f64;
            samples += 3;
            x += x_step;
        }
        y += y_step;
    }

    if samples == 0 {
        255.0
    } else {
        (diff / samples as f64) as f32
    }
}

fn add_candidate(candidates: &mut Vec<OverlapCandidate>, overlap: i32, row_score: f32) {
    if let Some(candidate) = candidates
        .iter_mut()
        .find(|candidate| candidate.overlap == overlap)
    {
        candidate.row_score = candidate.row_score.min(row_score);
        return;
    }
    candidates.push(OverlapCandidate {
        overlap,
        row_score,
        pixel_score: 255.0,
    });
}
