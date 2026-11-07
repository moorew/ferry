import { Stack } from 'expo-router';

import { useClips } from '../../../core/hooks';
import { t } from '../../../strings';
import { ClipList } from '../../../ui/ClipList';

export default function Pinned() {
  const [clips] = useClips('is:pinned');
  return (
    <>
      <Stack.Screen options={{ title: t('tab.pinned') }} />
      <ClipList clips={clips} empty={{ title: t('pinned.empty.title'), body: t('pinned.empty.body') }} />
    </>
  );
}
