<<<<<<< HEAD
with wave_size as (
    select
        count(*) as n_pages
    from
        "status" join waves using (user_id)
    where
        search_status = 'closed' and wave_name = $1::text
), deleted as (
    delete from
        waves 
    where
        wave_name = $1::text
    returning
        wave_id
) select
    n_pages as "n_pages!",
    wave_id
from
    wave_size full join deleted on true
=======
delete from waves where wave_name = $1::text returning wave_id;
>>>>>>> b160a62f64d6fddae428d3c2192dde188c6b13d5
