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
