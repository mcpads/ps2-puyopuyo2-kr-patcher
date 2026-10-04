use std::collections::VecDeque;

use super::*;

pub(super) fn clear_border_connected_light_neutral(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    minimum_channel: u8,
    maximum_channel_spread: u8,
) -> Result<usize> {
    ensure!(
        width > 0 && height > 0 && pixels.len() == width * height * 4,
        "stylized artwork background-removal dimensions changed"
    );

    let mut connected = vec![false; width * height];
    let mut queue = VecDeque::new();
    for x in 0..width {
        enqueue_if_light_neutral(
            pixels,
            width,
            x,
            0,
            minimum_channel,
            maximum_channel_spread,
            &mut connected,
            &mut queue,
        );
        if height > 1 {
            enqueue_if_light_neutral(
                pixels,
                width,
                x,
                height - 1,
                minimum_channel,
                maximum_channel_spread,
                &mut connected,
                &mut queue,
            );
        }
    }
    for y in 1..height.saturating_sub(1) {
        enqueue_if_light_neutral(
            pixels,
            width,
            0,
            y,
            minimum_channel,
            maximum_channel_spread,
            &mut connected,
            &mut queue,
        );
        if width > 1 {
            enqueue_if_light_neutral(
                pixels,
                width,
                width - 1,
                y,
                minimum_channel,
                maximum_channel_spread,
                &mut connected,
                &mut queue,
            );
        }
    }

    let mut removed = 0;
    while let Some((x, y)) = queue.pop_front() {
        let index = y * width + x;
        pixels[index * 4..index * 4 + 4].copy_from_slice(&[0, 0, 0, 0]);
        removed += 1;

        if x > 0 {
            enqueue_if_light_neutral(
                pixels,
                width,
                x - 1,
                y,
                minimum_channel,
                maximum_channel_spread,
                &mut connected,
                &mut queue,
            );
        }
        if x + 1 < width {
            enqueue_if_light_neutral(
                pixels,
                width,
                x + 1,
                y,
                minimum_channel,
                maximum_channel_spread,
                &mut connected,
                &mut queue,
            );
        }
        if y > 0 {
            enqueue_if_light_neutral(
                pixels,
                width,
                x,
                y - 1,
                minimum_channel,
                maximum_channel_spread,
                &mut connected,
                &mut queue,
            );
        }
        if y + 1 < height {
            enqueue_if_light_neutral(
                pixels,
                width,
                x,
                y + 1,
                minimum_channel,
                maximum_channel_spread,
                &mut connected,
                &mut queue,
            );
        }
    }
    Ok(removed)
}

#[allow(clippy::too_many_arguments)]
fn enqueue_if_light_neutral(
    pixels: &[u8],
    width: usize,
    x: usize,
    y: usize,
    minimum_channel: u8,
    maximum_channel_spread: u8,
    connected: &mut [bool],
    queue: &mut VecDeque<(usize, usize)>,
) {
    let index = y * width + x;
    if !connected[index]
        && is_light_neutral(
            &pixels[index * 4..index * 4 + 4],
            minimum_channel,
            maximum_channel_spread,
        )
    {
        connected[index] = true;
        queue.push_back((x, y));
    }
}

fn is_light_neutral(pixel: &[u8], minimum_channel: u8, maximum_channel_spread: u8) -> bool {
    let channels = [pixel[0], pixel[1], pixel[2]];
    let minimum = channels.into_iter().min().expect("three RGB channels");
    let maximum = channels.into_iter().max().expect("three RGB channels");
    pixel[3] != 0 && minimum >= minimum_channel && maximum - minimum <= maximum_channel_spread
}
