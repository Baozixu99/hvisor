DTS_FILES := $(wildcard *.dts)
DTB_FILES := $(DTS_FILES:.dts=.dtb)

all: $(DTB_FILES)
%.dtb: %.dts
	dtc -I dts -O dtb $< -o $@
cp:
	cp e2000q-linux1.dtb  /mnt/d/Desktop/tftp/e2000q/
	cp e2000q-linux2.dtb  /mnt/d/Desktop/tftp/e2000q/zone1/
clean:
	rm -f $(DTB_FILES)