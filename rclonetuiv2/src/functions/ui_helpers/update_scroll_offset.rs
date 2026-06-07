pub fn update_scroll_offset(selected_idx: usize, mut scroll_offset: usize, list_h: usize, total_items: usize) -> usize {
    if total_items == 0 || list_h == 0 {
        return 0;
    }
    if selected_idx < scroll_offset {
        scroll_offset = selected_idx;
    } else if selected_idx >= scroll_offset + list_h {
        scroll_offset = selected_idx.saturating_sub(list_h).saturating_add(1);
    }
    let max_offset = total_items.saturating_sub(list_h);
    if scroll_offset > max_offset {
        scroll_offset = max_offset;
    }
    scroll_offset
}
