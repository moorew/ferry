// Android: no bottom bar; History is home and the rest open from it (SPEC 21.5).

import { Stack } from 'expo-router';

export default function Screens() {
  return <Stack screenOptions={{ headerShown: false }} />;
}
