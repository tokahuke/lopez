-- TODO see https://stackoverflow.com/questions/34708509/how-to-use-returning-with-on-conflict-in-postgresql
with inserted as (
    insert into
        waves (wave_name)
    values
        ($1::text)
    on conflict (wave_name) do nothing
    returning
        wave_id
) select * from inserted
union
select
    wave_id
from
    waves
where
    wave_name = $1::text
