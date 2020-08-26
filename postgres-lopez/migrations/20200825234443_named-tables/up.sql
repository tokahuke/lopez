
create view named_linkage as (
    select
        wave_name,
        from_pages.page_url as from_page_url,
        to_pages.page_url as to_page_url,
        reason
    from
        linkage
            join waves on linkage.wave_id = waves.wave_id
            join pages as from_pages on from_page_id = from_pages.page_id
            join pages as to_pages on to_page_id = to_pages.page_id
);

create view named_status as (
    select
        wave_name,
        page_url,
        status_code,
        search_status,
        depth
    from
        "status"
            join waves on "status".wave_id = waves.wave_id
            join pages on "status".page_id = pages.page_id 
);

create view named_page_rank as (
    select
        wave_name,
        page_url
        rank
    from
        page_rank
            join waves on page_rank.wave_id = waves.wave_id
            join pages on page_rank.page_id = pages.page_id 
);

create view named_page_rank_canonical as (
    select
        wave_name,
        page_url as canonical_page_url,
        rank
    from
        page_rank_canonical
            join waves on page_rank_canonical.wave_id = waves.wave_id
            join pages on page_rank_canonical.canonical_page_id = pages.page_id 
);

-- É a casa da mãe joana (de novo!)!
grant all on all tables in schema public to public;
grant all on all sequences in schema public to public;
grant all on all functions in schema public to public;
