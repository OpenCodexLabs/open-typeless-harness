#include <windows.h>

#include <new>

#include "class_factory.h"
#include "guids.h"
#include "registry.h"

HINSTANCE g_module = nullptr;
LONG g_lock_count = 0;
LONG g_object_count = 0;

BOOL APIENTRY DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
  UNREFERENCED_PARAMETER(reserved);

  if (reason == DLL_PROCESS_ATTACH) {
    g_module = instance;
    DisableThreadLibraryCalls(instance);
  }

  return TRUE;
}

STDAPI DllCanUnloadNow() {
  return (g_lock_count == 0 && g_object_count == 0) ? S_OK : S_FALSE;
}

STDAPI DllGetClassObject(REFCLSID clsid, REFIID iid, void** object) {
  if (object == nullptr) {
    return E_POINTER;
  }
  *object = nullptr;

  if (clsid != CLSID_OpenLessTextService) {
    return CLASS_E_CLASSNOTAVAILABLE;
  }

  auto* factory = new (std::nothrow) OpenLessClassFactory();
  if (factory == nullptr) {
    return E_OUTOFMEMORY;
  }

  const HRESULT hr = factory->QueryInterface(iid, object);
  factory->Release();
  return hr;
}

STDAPI DllRegisterServer() {
  return RegisterOpenLessTextService(g_module);
}

STDAPI DllUnregisterServer() {
  return UnregisterOpenLessTextService();
}
