select
    count(*) as crawled
from
    "status"
where
    search_status = 'closed' and wave_id = $1::integer
