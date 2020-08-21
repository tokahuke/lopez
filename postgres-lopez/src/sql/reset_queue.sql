update
    status
set
    search_status = 'open'
where
    wave_id = $1::integer and search_status in ('taken', 'error');
