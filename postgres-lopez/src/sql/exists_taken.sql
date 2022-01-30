select exists (
    select from
        "status"
    where
        wave_id = $1::integer
            and search_status = 'taken'
) as exists_taken
