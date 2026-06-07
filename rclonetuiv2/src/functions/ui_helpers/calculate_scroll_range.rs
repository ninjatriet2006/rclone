pub fn calculate_scroll_range(selected_idx: usize, total_items: usize, height: usize) -> std::ops::Range<usize> {
    if total_items == 0 || height == 0 {
        return 0..0;
    }
    let scroll_offset = if selected_idx < height / 2 {
        0
    } else if selected_idx + height / 2 >= total_items {
        total_items.saturating_sub(height)
    } else {
        selected_idx - height / 2
    };
    let end = std::cmp::min(total_items, scroll_offset + height);
    scroll_offset..end
}
