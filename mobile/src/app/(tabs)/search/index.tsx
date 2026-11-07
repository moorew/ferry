// Search (iOS 27: a tab in the tab bar). Same syntax as the desktop picker:
// free text, from:<device>, is:pinned, is:image, is:link, today.

import { Stack } from 'expo-router';
import { useState } from 'react';

import { useClips } from '../../../core/hooks';
import { t } from '../../../strings';
import { ClipList } from '../../../ui/ClipList';

export default function Search() {
  const [query, setQuery] = useState('');
  const [clips] = useClips(query);
  return (
    <>
      <Stack.Screen
        options={{
          title: t('tab.search'),
          headerSearchBarOptions: {
            placeholder: t('search.placeholder'),
            onChangeText: (e) => setQuery(e.nativeEvent.text),
            autoCapitalize: 'none',
          },
        }}
      />
      <ClipList clips={query ? clips : []} empty={{ title: query ? t('search.empty', { query }) : t('search.placeholder') }} />
    </>
  );
}
