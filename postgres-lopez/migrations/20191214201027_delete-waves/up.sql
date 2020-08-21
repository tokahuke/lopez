alter table linkage drop constraint linkage_wave_id_fkey;
alter table
    linkage
add
    foreign key (wave_id)
    references waves (wave_id)
    on delete cascade;

alter table status drop constraint status_wave_id_fkey;
alter table
    status
add
    foreign key (wave_id)
    references waves (wave_id)
    on delete cascade;

alter table on_page drop constraint on_page_wave_id_fkey;
alter table
    on_page
add
    foreign key (wave_id)
    references waves (wave_id)
    on delete cascade;

alter table page_rank_canonical drop constraint page_rank_canonical_wave_id_fkey;
alter table
    page_rank_canonical
add
    foreign key (wave_id)
    references waves (wave_id)
    on delete cascade;

create or replace function pages_garbage_collect() returns void as $gc$
    begin
        execute (
            with relations as (
                select
                    relname,
                    attname
                from
                    pg_class join pg_attribute on attrelid = oid
                where
                    attname ~ '(^|_)page_id$'
                        and relkind = 'r'
                        and relname != 'pages'
            ), find_unused as (
                select
                    $$
                        select
                            pages.page_id
                        from
                            pages left join $$ || relname || $$
                                on pages.page_id = $$ || relname || $$.$$ || attname || $$
                        where
                            $$ || relname || $$.$$ || attname || $$ is null
                    $$ as find_unused_query
                from
                    relations
            ) select
                $$
                    with unused_pages as (
                        $$ || string_agg(find_unused_query, 'intersect') || $$
                    ) delete from
                        pages
                    using
                        unused_pages
                    where
                        pages.page_id = unused_pages.page_id
                $$
            from
                find_unused
        );
    end;
$gc$ language plpgsql;

create or replace function pages_garbage_collect_trgger() returns trigger as $$
    begin
        execute pages_garbage_collect();
        return new;
    end;
$$ language plpgsql;

create trigger pages_garbage_collect_trgger
    after delete or truncate on waves
    for each statement
    execute function pages_garbage_collect_trgger();
