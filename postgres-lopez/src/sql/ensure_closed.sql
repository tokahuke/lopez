update
    status
set
    status_code = $3::integer,
    search_status = 'closed'
where
    wave_id = $1::integer and page_id = $2::bigint;