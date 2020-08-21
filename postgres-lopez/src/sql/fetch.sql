with to_take as (
    select
        page_id,
        page_url
    from
        "status" join pages using (page_id)
    where
        wave_id = $1::integer
            and search_status = 'open'
            and depth <= $3::smallint
            
    order by
        depth asc
    limit
        $2::bigint
) update
    "status"
set
    search_status = 'taken'
from
    to_take
where
    wave_id = $1::integer
        and to_take.page_id = "status".page_id
returning
    page_url,
    depth;
