insert into
    pages (page_id, page_url)
select
    *
from
    unnest($1::bigint[], $2::text[]) as _ (page_id, page_url)
on conflict do nothing;
