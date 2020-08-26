select 
    from_page_id,
    to_page_id
from
    linkage
        join "status" as from_status 
            on from_status.page_id = from_page_id 
                and from_status.wave_id = linkage.wave_id
        join "status" as to_status
            on to_status.page_id = to_page_id
                and to_status.wave_id = linkage.wave_id
where
    linkage.wave_id = $1::integer
        and linkage.reason = 'ahref'
        and from_status.search_status = 'closed'
        and to_status.search_status = 'closed';
