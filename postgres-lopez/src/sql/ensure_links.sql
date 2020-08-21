insert into
    linkage (wave_id, from_page_id, to_page_id, reason)
select
    $1::integer,
    $2::bigint,
    to_page_id,
    reason::reason_enum
from
    unnest($3::bigint[], $4::text[]) as _ (to_page_id, reason)
on conflict do nothing;
