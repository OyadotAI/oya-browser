# Registers Oya as a candidate in Windows Default Apps. Never writes UserChoice or takes HTTP defaults.
!macro customInstall
  WriteRegStr SHCTX "Software\Classes\OyaBrowserURL" "" "Oya Browser URL"
  WriteRegStr SHCTX "Software\Classes\OyaBrowserURL" "URL Protocol" ""
  WriteRegStr SHCTX "Software\Classes\OyaBrowserURL\DefaultIcon" "" '$"$INSTDIR\${APP_EXECUTABLE_FILENAME}$",0'
  WriteRegStr SHCTX "Software\Classes\OyaBrowserURL\shell\open\command" "" '$"$INSTDIR\${APP_EXECUTABLE_FILENAME}$" $"%1$"'
  WriteRegStr SHCTX "Software\Clients\StartMenuInternet\OyaBrowser" "" "Oya Browser"
  WriteRegStr SHCTX "Software\Clients\StartMenuInternet\OyaBrowser\shell\open\command" "" '$"$INSTDIR\${APP_EXECUTABLE_FILENAME}$"'
  WriteRegStr SHCTX "Software\Clients\StartMenuInternet\OyaBrowser\Capabilities" "ApplicationName" "Oya Browser"
  WriteRegStr SHCTX "Software\Clients\StartMenuInternet\OyaBrowser\Capabilities" "ApplicationDescription" "Browse the web with Oya Browser"
  WriteRegStr SHCTX "Software\Clients\StartMenuInternet\OyaBrowser\Capabilities\URLAssociations" "http" "OyaBrowserURL"
  WriteRegStr SHCTX "Software\Clients\StartMenuInternet\OyaBrowser\Capabilities\URLAssociations" "https" "OyaBrowserURL"
  WriteRegStr SHCTX "Software\RegisteredApplications" "Oya Browser" "Software\Clients\StartMenuInternet\OyaBrowser\Capabilities"
!macroend

# Only remove Oya-owned registration, not another browser's associations.
!macro customUnInstall
  DeleteRegValue SHCTX "Software\RegisteredApplications" "Oya Browser"
  DeleteRegKey SHCTX "Software\Clients\StartMenuInternet\OyaBrowser"
  DeleteRegKey SHCTX "Software\Classes\OyaBrowserURL"
!macroend
