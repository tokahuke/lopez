insert into
    page_rank_canonical (wave_id, canonical_page_id, rank)
select
    $1::integer, *
from
    unnest($2::bigint[], $3::float[])
on conflict do nothing;
