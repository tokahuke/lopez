select
    count(*) as crawled
from
    "status"
where
    search_status in ('closed', 'error') and wave_id = $1::integer
