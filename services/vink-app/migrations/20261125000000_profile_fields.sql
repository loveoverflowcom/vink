-- Profile edits have their own CAS. Handle receipts, quota and version remain independent.
ALTER TABLE author_profiles
    ADD COLUMN profile_version bigint NOT NULL DEFAULT 0 CHECK (profile_version >= 0),
    ADD COLUMN bio text CHECK (bio IS NULL OR char_length(bio) BETWEEN 1 AND 500),
    ADD COLUMN policies jsonb NOT NULL DEFAULT '{
        "pen_name":"everyone", "avatar":"everyone", "bio":"everyone", "date_of_birth":"self",
        "followers_list":"everyone", "followers_count":"everyone",
        "following_list":"self", "following_count":"self"
    }' CHECK (
        jsonb_typeof(policies) = 'object'
        AND policies ?& ARRAY['pen_name','avatar','bio','date_of_birth','followers_list',
                              'followers_count','following_list','following_count']
        AND policies - ARRAY['pen_name','avatar','bio','date_of_birth','followers_list',
                             'followers_count','following_list','following_count'] = '{}'::jsonb
        AND policies->'pen_name' = '"everyone"'::jsonb
        AND policies->'date_of_birth' = '"self"'::jsonb
        AND policies->'avatar' IN ('"everyone"'::jsonb, '"self"'::jsonb)
        AND policies->'bio' IN ('"everyone"'::jsonb, '"self"'::jsonb)
        AND policies->'followers_list' IN ('"everyone"'::jsonb, '"self"'::jsonb)
        AND policies->'followers_count' IN ('"everyone"'::jsonb, '"self"'::jsonb)
        AND policies->'following_list' IN ('"everyone"'::jsonb, '"self"'::jsonb)
        AND policies->'following_count' IN ('"everyone"'::jsonb, '"self"'::jsonb)
    );
