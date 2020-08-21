update
    status
set
    search_status = 'error'
where
    wave_id = $1::integer and page_id = $2::bigint;