with numbered as (
    -- Get a bunch of open low-depth pages.
    select
        page_id,
        page_url,
        depth,
        count(*) over (
            partition by substring(page_url from '^https?://([^/]*)/')
            order by depth
        ) as count
    from
        "status" join pages using (page_id)
    where
        wave_id = $1::integer
            and search_status = 'open'
            and depth <= $3::smallint
), to_take as (
    -- From the pages, get the ones that are low count. This ensures a
    -- plurality of domanins in each batch.
    select
        page_id,
        page_url
    from
        numbered
    order by
        count,
        depth
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
