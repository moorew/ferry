// tailcopy://copy-latest, from the Shortcuts action (SPEC 21.4).

import { router } from 'expo-router';
import { useEffect } from 'react';

import { Core } from '../../modules/tailcopy-core';
import { t } from '../strings';
import { useToast } from '../ui/Toast';

export default function CopyLatest() {
  const toast = useToast();
  useEffect(() => {
    (async () => {
      const [latest] = await Core.list('', 0, 1, 0, 0);
      const ok = latest ? await Core.copy(latest.id, false) : false;
      toast(t(ok ? 'toast.copied' : 'toast.empty'));
      router.replace('/');
    })();
  }, [toast]);
  return null;
}
