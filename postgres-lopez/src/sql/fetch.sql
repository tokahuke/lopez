with to_take_raw as (
    select
        page_id,
        page_url,
        depth
    from
        "status" join pages using (page_id)
    where
        wave_id = $1::integer
            and search_status = 'open'
            and depth <= $3::smallint
    limit
        10 * $2::bigint
), numbered as (
    select
        *,
        count(*) over (
            partition by substring(page_url from '^https?://[^/]*/')
            order by depth
        ) as count
    from
        to_take_raw
), to_take as (
    select
        page_id,
        page_url
    from
        numbered
    order by
        count
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
