insert into
    status (wave_id, page_id, search_status, depth)
select
    $1::integer,
    page_id,
    'open',
    $3::smallint
from
    unnest($2::bigint[]) as _ (page_id)
on conflict do nothing;
-- from (
--     -- page_id has to be unique for ON CONFLICT.
--     select distinct * from unnest($2::bigint[]) as _ (page_id)
-- ) as _
-- on conflict (wave_id, page_id) do update set
--     depth = least(status.depth, $3::smallint)
-- where
--     status.search_status = 'open';
